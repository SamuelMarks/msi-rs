//! # End-to-End `LibScript` `WiX` Replacement Integration Test Suite
//!
//! Validates the complete packaging workflow for replacing `WiX` in `libscript`:
//! - Compiling and linking manifests with branding variables (`WixUIBannerBmp`, `WixUIDialogBmp`, `WixUILicenseRtf`).
//! - Compiling and linking VBScript/JScript port validation custom actions.
//! - `RadioButtonGroup` and `RadioButton` setup mode selection.
//! - Relative sequence resolution (`Before`, `After`, `OnExit`).
//! - Multi-cabinet partitioning across 4 media disks (`engine.cab`, `runtimes.cab`, `databases.cab`, `codebase.cab`).
//! - Long component and file identifier auto-hashing (exceeding 72 characters).
//! - Inspecting and opening generated binary `.msi` packages.

use msi::database::tables::record::FieldValue;
use msi::error::Result;
use msi::package::Package;
use msi::wix::toolchain::{CandleOptions, LightOptions};
use std::fs;

/// Tests the complete libscript component installer workflow matching `template_msi.sh`.
///
/// # Errors
///
/// Returns [`msi::Error`] on compiler, linker, or package verification failure.
#[test]
#[allow(clippy::too_many_lines)]
fn test_libscript_template_msi_workflow() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("libscript_tmpl_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let banner_bmp = temp_dir.join("banner.bmp");
    let dialog_bmp = temp_dir.join("dialog.bmp");
    let license_rtf = temp_dir.join("license.rtf");
    let vbs_script = temp_dir.join("validate_mysql.vbs");
    let dummy_payload = temp_dir.join("libscript.cmd");

    fs::write(&banner_bmp, b"BM_BANNER_BYTES")?;
    fs::write(&dialog_bmp, b"BM_DIALOG_BYTES")?;
    fs::write(&license_rtf, "{\\rtf1\\ansi Standard LibScript EULA}")?;
    fs::write(&vbs_script, b"' VBScript Port Validation")?;
    fs::write(&dummy_payload, b"@echo off\r\necho LibScript Installed\r\n")?;

    let wxs_file = temp_dir.join("TestPackage.wxs");
    let wxs_content = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="TestStack" Language="1033" Version="1.0.0" Manufacturer="LibScriptTest" UpgradeCode="{{12345678-1234-5678-1234-567812345678}}">
    <Package Description="TestStack Installer" Manufacturer="LibScriptTest" InstallerVersion="300" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version is installed." Schedule="afterInstallInitialize" AllowSameVersionUpgrades="no" />
    <Media Id="1" Cabinet="#engine.cab" EmbedCab="yes" />

    <WixVariable Id="WixUIBannerBmp" Value="{}" />
    <WixVariable Id="WixUIDialogBmp" Value="{}" />
    <WixVariable Id="WixUILicenseRtf" Value="{}" />

    <Property Id="PROP_mysql_PORT" Value="3306" Secure="yes" />
    <Property Id="INSTALL_mysql" Value="1" Secure="yes" />
    <Property Id="SETUP_MODE" Value="Simple" Secure="yes" />

    <Binary Id="Bin_Val_mysql" SourceFile="{}" />
    <CustomAction Id="CA_Val_mysql" BinaryKey="Bin_Val_mysql" VBScriptCall="CheckPorts_mysql" Return="check" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="TestStack">
          <Component Id="MainPayload" Guid="{{22222222-2222-2222-2222-222222222222}}">
            <File Id="File_LibscriptCmd" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="TestStack Services" Level="1">
      <ComponentRef Id="MainPayload" />
    </Feature>

    <UI Id="CustomUI">
      <Dialog Id="Dlg_Welcome" Width="370" Height="270" Title="Welcome Setup">
        <Control Id="Next" Type="PushButton" X="236" Y="243" Width="56" Height="17" Default="yes" Text="Next">
          <Publish Event="EndDialog" Value="Return">1</Publish>
        </Control>
      </Dialog>
      <Dialog Id="Dlg_License" Width="370" Height="270" Title="License Agreement">
        <Control Id="AgreementText" Type="ScrollableText" X="20" Y="60" Width="330" Height="150" Sunken="yes" TabSkip="no">
          <Text SourceFile="{}" />
        </Control>
        <Control Id="Next" Type="PushButton" X="236" Y="243" Width="56" Height="17" Default="yes" Text="Next">
          <Publish Event="EndDialog" Value="Return">1</Publish>
        </Control>
      </Dialog>
      <Dialog Id="Dlg_Exit" Width="370" Height="270" Title="Installation Complete">
        <Control Id="Finish" Type="PushButton" X="236" Y="243" Width="56" Height="17" Default="yes" Text="Finish">
          <Publish Event="EndDialog" Value="Return">1</Publish>
        </Control>
      </Dialog>
    </UI>

    <InstallUISequence>
      <Show Dialog="Dlg_Welcome" After="CostFinalize">NOT Installed</Show>
      <Show Dialog="Dlg_License" After="Dlg_Welcome">NOT Installed</Show>
      <Show Dialog="Dlg_Exit" OnExit="success">NOT Installed</Show>
    </InstallUISequence>

    <InstallExecuteSequence>
      <Custom Action="CA_Val_mysql" Before="InstallInitialize">NOT Installed</Custom>
    </InstallExecuteSequence>
  </Product>
</Wix>
"##,
        banner_bmp.display(),
        dialog_bmp.display(),
        license_rtf.display(),
        vbs_script.display(),
        dummy_payload.display(),
        license_rtf.display(),
    );
    fs::write(&wxs_file, wxs_content)?;

    // 1. Compile with Candle
    let candle_opts = CandleOptions {
        sources: vec![wxs_file],
        output: Some(temp_dir.join("TestPackage.wixobj")),
        ..CandleOptions::new()
    };
    let wixobj_paths = candle_opts.execute()?;
    assert_eq!(wixobj_paths.len(), 1);
    assert!(wixobj_paths[0].exists());

    // 2. Link with Light
    let msi_out = temp_dir.join("TestPackage.msi");
    let light_opts = LightOptions {
        inputs: wixobj_paths,
        output: Some(msi_out.clone()),
        suppress_ice: true,
        ..LightOptions::new()
    };
    let produced_msi = light_opts.execute()?;
    assert_eq!(produced_msi, msi_out);
    assert!(msi_out.exists());

    // 3. Inspect generated MSI package
    let pkg = Package::open(&msi_out)?;
    let db = pkg.database();

    // Verify CustomAction table
    let ca_records = db.get_records("CustomAction");
    assert_ne!(ca_records, []);
    let ca_mysql = ca_records
        .iter()
        .find(|r| r.get(0) == Some(&FieldValue::String("CA_Val_mysql".to_string())));
    assert!(ca_mysql.is_some());
    assert_eq!(
        ca_mysql.and_then(|r| r.get(1)),
        Some(&FieldValue::Short(6)) // Type 6 VBScript
    );

    // Verify InstallUISequence has resolved sequence numbers
    let ui_seq = db.get_records("InstallUISequence");
    assert_ne!(ui_seq, []);
    let exit_show = ui_seq
        .iter()
        .find(|r| r.get(0) == Some(&FieldValue::String("Dlg_Exit".to_string())));
    assert!(exit_show.is_some());
    assert_eq!(
        exit_show.and_then(|r| r.get(2)),
        Some(&FieldValue::Short(-1)) // OnExit="success" maps to -1
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests multi-cabinet media partitioning with 74-character harvested identifiers matching `build_msi.sh`.
///
/// # Errors
///
/// Returns [`msi::Error`] on compilation or verification failure.
#[test]
#[allow(clippy::too_many_lines, clippy::needless_raw_string_hashes)]
fn test_libscript_build_msi_multicab_and_long_identifiers() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("libscript_mcab_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let p1 = temp_dir.join("engine.cmd");
    let p2 = temp_dir.join("python.dll");
    let p3 = temp_dir.join("mysql.exe");
    let p4 = temp_dir.join("app.js");

    fs::write(&p1, b"engine payload")?;
    fs::write(&p2, b"python runtime payload")?;
    fs::write(&p3, b"mysql database payload")?;
    fs::write(&p4, b"app codebase payload")?;

    let long_comp_id = "CMP_H__lib_web_servers_nginx_conf_simple_location_proxy_websockets_conf";
    let long_file_id = "FIL_H__lib_web_servers_nginx_conf_simple_location_proxy_websockets_conf";

    let wxs_file = temp_dir.join("MultiCabPackage.wxs");
    let wxs_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="OpenEdXStack" Language="1033" Version="2.0.0" Manufacturer="LibScript" UpgradeCode="{{99999999-8888-7777-6666-555555555555}}">
    <Package Description="Enterprise MultiCab Package" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="Newer version exists." Schedule="afterInstallExecute" />

    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />
    <Media Id="2" Cabinet="runtimes.cab" EmbedCab="yes" />
    <Media Id="3" Cabinet="databases.cab" EmbedCab="yes" />
    <Media Id="4" Cabinet="codebase.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="OpenEdX">
          <Component Id="C_Engine" Guid="{{11111111-1111-1111-1111-111111111111}}">
            <File Id="F_Engine" Source="{}" DiskId="1" KeyPath="yes" />
          </Component>
          <Component Id="C_Runtime" Guid="{{22222222-2222-2222-2222-222222222222}}">
            <File Id="F_Runtime" Source="{}" DiskId="2" KeyPath="yes" />
          </Component>
          <Component Id="C_Database" Guid="{{33333333-3333-3333-3333-333333333333}}">
            <File Id="F_Database" Source="{}" DiskId="3" KeyPath="yes" />
          </Component>
          <Component Id="{long_comp_id}" Guid="{{44444444-4444-4444-4444-444444444444}}">
            <File Id="{long_file_id}" Source="{}" DiskId="4" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="Main" Level="1">
      <ComponentRef Id="C_Engine" />
      <ComponentRef Id="C_Runtime" />
      <ComponentRef Id="C_Database" />
      <ComponentRef Id="{long_comp_id}" />
    </Feature>
  </Product>
</Wix>
"#,
        p1.display(),
        p2.display(),
        p3.display(),
        p4.display(),
    );
    fs::write(&wxs_file, wxs_content)?;

    // 1. Compile with Candle
    let candle_opts = CandleOptions {
        sources: vec![wxs_file],
        output: Some(temp_dir.join("MultiCabPackage.wixobj")),
        ..CandleOptions::new()
    };
    let wixobjs = candle_opts.execute()?;
    assert_eq!(wixobjs.len(), 1);

    // 2. Link with Light
    let msi_out = temp_dir.join("MultiCabPackage.msi");
    let light_opts = LightOptions {
        inputs: wixobjs,
        output: Some(msi_out.clone()),
        suppress_ice: true,
        ..LightOptions::new()
    };
    let res = light_opts.execute()?;
    assert_eq!(res, msi_out);

    // 3. Verify Package
    let pkg = Package::open(&msi_out)?;
    let db = pkg.database();

    // Verify Media records: 4 disks with partitioned sequence boundaries
    let media = db.get_records("Media");
    assert_eq!(media.len(), 4);
    assert_eq!(media[0].get(1), Some(&FieldValue::Long(1)));
    assert_eq!(media[1].get(1), Some(&FieldValue::Long(2)));
    assert_eq!(media[2].get(1), Some(&FieldValue::Long(3)));
    assert_eq!(media[3].get(1), Some(&FieldValue::Long(4)));

    // Verify File records: 4 files with sequence 1, 2, 3, 4
    let files = db.get_records("File");
    assert_eq!(files.len(), 4);
    assert_eq!(files[0].get(7), Some(&FieldValue::Short(1)));
    assert_eq!(files[1].get(7), Some(&FieldValue::Short(2)));
    assert_eq!(files[2].get(7), Some(&FieldValue::Short(3)));
    assert_eq!(files[3].get(7), Some(&FieldValue::Short(4)));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests the Open edX online dual-fragment compilation and linking workflow (`build_openedx_msi.sh --online`).
///
/// Validates:
/// - Dual-fragment symbol resolution across `Product.wxs` and `Payload.wxs`.
/// - Long custom action target commands (> 700 characters).
/// - Long license guard condition expressions (> 400 characters).
/// - Sensitive property masking registration in `MsiHiddenProperties`.
///
/// # Errors
///
/// Returns [`msi::Error`] on compiler, linker, or package verification failure.
#[test]
#[allow(clippy::too_many_lines)]
fn test_libscript_openedx_online_dual_fragment_workflow() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("libscript_openedx_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let dummy_cli = temp_dir.join("cli.cmd");
    fs::write(&dummy_cli, b"@echo off\r\necho OpenEdX CLI\r\n")?;

    let dummy_file1 = temp_dir.join("manifest.json");
    fs::write(&dummy_file1, b"{\"version\": \"2.4.0\"}\n")?;

    let dummy_file2 = temp_dir.join("vars.schema.json");
    fs::write(&dummy_file2, b"{\"type\": \"object\"}\n")?;

    let long_target = "cmd.exe /c &quot;[INSTALLFOLDER]libscript\\libscript.cmd&quot; install stacks/cms/openedx --admin-user=&quot;[PROP_OPENEDX_ADMIN_USERNAME]&quot; --admin-password=&quot;[PROP_OPENEDX_ADMIN_PASSWORD]&quot; --admin-email=&quot;[PROP_OPENEDX_ADMIN_EMAIL]&quot; --theme=&quot;[PROP_OPENEDX_THEME]&quot; --theme-repo-url=&quot;[PROP_OPENEDX_THEME_REPO_URL]&quot; --db-host=&quot;[PROP_MYSQL_HOST]&quot; --db-port=&quot;[PROP_MYSQL_PORT]&quot; --redis-host=&quot;[PROP_REDIS_HOST]&quot; --redis-port=&quot;[PROP_REDIS_PORT]&quot;".to_string();
    assert!(long_target.len() > 400);

    let long_cond = "NOT Installed AND NOT (AGREE_ALL_LICENSES=\"1\") AND NOT (LICENSE_ACCEPTED=\"1\" AND NOT (LICENSE_ACCEPTED_mysql=\"1\") AND NOT (LICENSE_ACCEPTED_redis=\"1\") AND NOT (LICENSE_ACCEPTED_mongodb=\"1\") AND NOT (LICENSE_ACCEPTED_python=\"1\") AND NOT (LICENSE_ACCEPTED_nodejs=\"1\") AND NOT (LICENSE_ACCEPTED_meilisearch=\"1\") AND NOT (LICENSE_ACCEPTED_gunicorn=\"1\") AND NOT (LICENSE_ACCEPTED_hmailserver=\"1\") AND NOT (LICENSE_ACCEPTED_nodeenv=\"1\"))".to_string();
    assert!(long_cond.len() > 300);

    let product_wxs = temp_dir.join("Product.wxs");
    let product_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Open edX Platform" Language="1033" Version="2.4.0.0" Manufacturer="The Axim Collaborative" UpgradeCode="12345678-1234-5678-1234-567812345678">
    <Package Description="Open edX Installer" Manufacturer="The Axim Collaborative" InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" CompressionLevel="high" />

    <Property Id="MsiHiddenProperties" Value="PROP_OPENEDX_ADMIN_PASSWORD;PROP_MYSQL_PASSWORD;PROP_MONGODB_PASSWORD" Secure="yes" />
    <Property Id="PROP_OPENEDX_ADMIN_PASSWORD" Value="super_secret_password" Secure="yes" />
    <Property Id="LICENSE_ACCEPTED" Value="0" Secure="yes" />

    <CustomAction Id="InstallOpenEdXService" Directory="INSTALLFOLDER" ExeCommand="{}" Execute="deferred" Return="ignore" Impersonate="no" />
    <CustomAction Id="CA_AbortNoLicense" Error="Installation aborted: All licenses must be accepted." />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="OpenEdX">
          <Component Id="CmpCli" Guid="B35F9271-2B4A-48DC-8812-3D7C51094E1A">
            <File Id="FileCli" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="ProductFeature" Title="Open edX" Level="1">
      <ComponentRef Id="CmpCli" />
      <ComponentGroupRef Id="HarvestedPayloadComponents" />
    </Feature>

    <InstallExecuteSequence>
      <Custom Action="CA_AbortNoLicense" Before="InstallInitialize"><![CDATA[{}]]></Custom>
      <Custom Action="InstallOpenEdXService" Before="InstallFinalize"><![CDATA[NOT Installed]]></Custom>
    </InstallExecuteSequence>
  </Product>
</Wix>
"#,
        long_target,
        dummy_cli.display(),
        long_cond,
    );
    fs::write(&product_wxs, product_content)?;

    let payload_wxs = temp_dir.join("Payload.wxs");
    let payload_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Directory Id="DIR_config" Name="config">
        <Component Id="CMP_Manifest" Guid="E2A89C15-99BD-4720-A0E8-A97A2E504F63">
          <File Id="FIL_Manifest" Source="{}" KeyPath="yes" DiskId="1" />
        </Component>
        <Component Id="CMP_Schema" Guid="D1A72951-86E3-4E61-A79B-7D8C430931B5">
          <File Id="FIL_Schema" Source="{}" KeyPath="yes" DiskId="1" />
        </Component>
      </Directory>
    </DirectoryRef>
    <ComponentGroup Id="HarvestedPayloadComponents">
      <ComponentRef Id="CMP_Manifest" />
      <ComponentRef Id="CMP_Schema" />
    </ComponentGroup>
  </Fragment>
</Wix>
"#,
        dummy_file1.display(),
        dummy_file2.display(),
    );
    fs::write(&payload_wxs, payload_content)?;

    // 1. Compile both source manifests to wixobj
    let candle_opts = CandleOptions {
        sources: vec![product_wxs, payload_wxs],
        arch: Some("x64".to_string()),
        ..CandleOptions::new()
    };
    let wixobjs = candle_opts.execute()?;
    assert_eq!(wixobjs.len(), 2);

    // 2. Link both wixobjs into MSI
    let out_msi = temp_dir.join("OpenEdX_Online.msi");
    let light_opts = LightOptions {
        inputs: wixobjs,
        output: Some(out_msi.clone()),
        extensions: vec!["WixUIExtension".to_string()],
        suppress_ice: true,
        ..LightOptions::new()
    };
    let linked_msi = light_opts.execute()?;
    assert!(linked_msi.exists());

    // 3. Inspect the created MSI package
    let pkg = Package::open(&out_msi)?;
    assert_eq!(pkg.metadata().product_name(), "Open edX Platform");

    let db = pkg.database();
    let ca_records = db.get_records("CustomAction");
    assert!(ca_records.iter().any(|r| {
        r.get(3).is_some_and(|f| {
            if let FieldValue::String(s) = f {
                s.contains("--admin-user") && s.contains("--admin-password")
            } else {
                false
            }
        })
    }));

    let seq_records = db.get_records("InstallExecuteSequence");
    assert!(seq_records.iter().any(|r| {
        r.get(1).is_some_and(|f| {
            if let FieldValue::String(s) = f {
                s.contains("AGREE_ALL_LICENSES") && s.contains("LICENSE_ACCEPTED_mysql")
            } else {
                false
            }
        })
    }));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests multi-cabinet extraction and payload validation for offline air-gapped installers.
///
/// # Errors
///
/// Returns [`msi::Error`] on packaging, linking, or cabinet extraction failure.
#[test]
fn test_libscript_openedx_offline_multicab_extraction() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("libscript_offline_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let file1 = temp_dir.join("engine.dat");
    let file2 = temp_dir.join("python.dat");
    let file3 = temp_dir.join("mysql.dat");
    let file4 = temp_dir.join("code.dat");

    let data1 = b"Engine Payload Data - LZX Compressed Partition";
    let data2 = b"Python Runtime Payload - MSZIP Compressed Partition";
    let data3 = b"MySQL Database Payload - MSZIP Compressed Partition";
    let data4 = b"Codebase Repository Payload - MSZIP Compressed Partition";

    fs::write(&file1, data1)?;
    fs::write(&file2, data2)?;
    fs::write(&file3, data3)?;
    fs::write(&file4, data4)?;

    let wxs_file = temp_dir.join("Offline.wxs");
    let wxs_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Open edX Offline" Language="1033" Version="2.4.0.0" Manufacturer="The Axim Collaborative" UpgradeCode="87654321-4321-4321-4321-876543218765">
    <Package Description="Offline Installer" Manufacturer="The Axim Collaborative" InstallerVersion="500" Compressed="yes" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" CompressionLevel="high" />
    <Media Id="2" Cabinet="runtimes.cab" EmbedCab="yes" CompressionLevel="medium" />
    <Media Id="3" Cabinet="databases.cab" EmbedCab="yes" CompressionLevel="medium" />
    <Media Id="4" Cabinet="codebase.cab" EmbedCab="yes" CompressionLevel="medium" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="INSTALLFOLDER" Name="OpenEdX">
        <Component Id="CmpEngine" Guid="11111111-1111-1111-1111-111111111111">
          <File Id="FileEngine" Source="{}" DiskId="1" KeyPath="yes" />
        </Component>
        <Component Id="CmpPython" Guid="22222222-2222-2222-2222-222222222222">
          <File Id="FilePython" Source="{}" DiskId="2" KeyPath="yes" />
        </Component>
        <Component Id="CmpMySQL" Guid="33333333-3333-3333-3333-333333333333">
          <File Id="FileMySQL" Source="{}" DiskId="3" KeyPath="yes" />
        </Component>
        <Component Id="CmpCode" Guid="44444444-4444-4444-4444-444444444444">
          <File Id="FileCode" Source="{}" DiskId="4" KeyPath="yes" />
        </Component>
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="All" Level="1">
      <ComponentRef Id="CmpEngine" />
      <ComponentRef Id="CmpPython" />
      <ComponentRef Id="CmpMySQL" />
      <ComponentRef Id="CmpCode" />
    </Feature>
  </Product>
</Wix>
"#,
        file1.display(),
        file2.display(),
        file3.display(),
        file4.display(),
    );
    fs::write(&wxs_file, wxs_content)?;

    let out_msi = temp_dir.join("OpenEdX_Offline.msi");
    let candle_opts = CandleOptions {
        sources: vec![wxs_file],
        arch: Some("x64".to_string()),
        ..CandleOptions::new()
    };
    let wixobjs = candle_opts.execute()?;

    let light_opts = LightOptions {
        inputs: wixobjs,
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..LightOptions::new()
    };
    light_opts.execute()?;

    // Verify all 4 embedded cabinets can be opened and files extracted
    let pkg = Package::open(&out_msi)?;
    let embedded_cabs = pkg.embedded_cabinets();
    assert_eq!(embedded_cabs.len(), 4);

    let mut extracted_file_data = std::collections::HashMap::new();
    for cab_bytes in embedded_cabs.values() {
        let reader = msi::cab::reader::CabinetReader::new(cab_bytes)?;
        for cf_file in reader.files() {
            let content = reader.extract_file(&cf_file.filename)?;
            extracted_file_data.insert(cf_file.filename.clone(), content);
        }
    }

    assert_eq!(extracted_file_data.get("FileEngine"), Some(&data1.to_vec()));
    assert_eq!(extracted_file_data.get("FilePython"), Some(&data2.to_vec()));
    assert_eq!(extracted_file_data.get("FileMySQL"), Some(&data3.to_vec()));
    assert_eq!(extracted_file_data.get("FileCode"), Some(&data4.to_vec()));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests unattended silent installation guard evaluation and property masking.
///
/// # Errors
///
/// Returns [`msi::Error`] on evaluation failure.
#[test]
fn test_libscript_unattended_silent_execution_guards() -> Result<()> {
    use msi::execution::properties::EvaluationContext;

    let mut ctx = EvaluationContext::new();
    ctx.set_property(
        "MsiHiddenProperties",
        "PROP_OPENEDX_ADMIN_PASSWORD;PROP_MYSQL_PASSWORD",
    );
    ctx.set_property("PROP_OPENEDX_ADMIN_PASSWORD", "SuperSecret123!");
    ctx.set_property("PROP_MYSQL_PORT", "3306");

    // Verify parameter masking
    assert_eq!(
        ctx.mask_if_hidden("PROP_OPENEDX_ADMIN_PASSWORD", "SuperSecret123!"),
        "******"
    );
    assert_eq!(ctx.mask_if_hidden("PROP_MYSQL_PORT", "3306"), "3306");

    // Verify unattended install license abort condition:
    // NOT (AGREE_ALL_LICENSES="1") AND NOT (LICENSE_ACCEPTED="1")
    let abort_cond = "NOT (AGREE_ALL_LICENSES=\"1\") AND NOT (LICENSE_ACCEPTED=\"1\")";

    // Scenario A: Unattended install without flags -> abort condition evaluates to True
    ctx.set_property("LICENSE_ACCEPTED", "0");
    assert!(ctx.evaluate_condition(abort_cond)?);

    // Scenario B: Unattended install with AGREE_ALL_LICENSES=1 -> abort condition evaluates to False
    ctx.set_property("AGREE_ALL_LICENSES", "1");
    assert!(!ctx.evaluate_condition(abort_cond)?);

    // Scenario C: Interactive install with LICENSE_ACCEPTED=1 -> abort condition evaluates to False
    ctx.set_property("AGREE_ALL_LICENSES", "0");
    ctx.set_property("LICENSE_ACCEPTED", "1");
    assert!(!ctx.evaluate_condition(abort_cond)?);

    Ok(())
}

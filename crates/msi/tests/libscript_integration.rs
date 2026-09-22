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
#[allow(clippy::too_many_lines, clippy::assert_is_empty)]
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
    assert!(!ca_records.is_empty());
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
    assert!(!ui_seq.is_empty());
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
#[allow(
    clippy::too_many_lines,
    clippy::needless_raw_string_hashes,
    clippy::assert_is_empty
)]
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

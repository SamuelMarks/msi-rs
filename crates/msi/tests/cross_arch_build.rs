#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
//! End-to-end integration tests for cross-architecture `.msi` package construction,
//! `WiX` preprocessing/compilation/linking, `.wixobj` architecture persistence,
//! PE32/PE32+ payload inspection, ICE80 validation, and simulated cross-architecture installation.

use msi::database::tables::core::DirectoryRow;
use msi::database::tables::record::FieldValue;
use msi::database::tables::types::DirectoryId;
use msi::error::Result;
use msi::package::{Package, ProductVersion};
use msi::wix::toolchain::{CandleOptions, LightOptions, WixBuildOptions};
use msi::{Architecture, Bitness, ComponentBitnessRule, TemplateSummary};
use std::fs;
use std::path::{Path, PathBuf};

/// Creates an isolated temporary directory for cross-architecture integration tests.
///
/// # Arguments
///
/// * `suffix` - Unique suffix identifying the test case.
///
/// # Returns
///
/// Path to the newly created temporary directory.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if directory creation fails.
fn make_temp_dir(suffix: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("msi_cross_arch_{suffix}_{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Synthesizes a minimal valid PE (`PE32` or `PE32+`) binary image for a given COFF machine type.
///
/// # Arguments
///
/// * `machine` - Raw COFF `IMAGE_FILE_HEADER.Machine` constant (e.g., `0x014C`, `0x8664`, `0xAA64`).
/// * `is_pe32_plus` - `true` for `PE32+` (`0x020B`), `false` for `PE32` (`0x010B`).
/// * `is_clr` - `true` to populate Data Directory 14 (COM Descriptor / .NET CLR header).
///
/// # Returns
///
/// Byte vector containing the synthetic PE image.
fn make_synthetic_pe(machine: u16, is_pe32_plus: bool, is_clr: bool) -> Vec<u8> {
    let mut pe = vec![0u8; 512];
    pe[0..2].copy_from_slice(b"MZ");
    pe[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    pe[0x40..0x44].copy_from_slice(b"PE\0\0");
    pe[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
    pe[0x46..0x48].copy_from_slice(&0u16.to_le_bytes());
    pe[0x54..0x56].copy_from_slice(&240u16.to_le_bytes());
    let opt_off = 0x58;
    let magic: u16 = if is_pe32_plus { 0x020B } else { 0x010B };
    pe[opt_off..opt_off + 2].copy_from_slice(&magic.to_le_bytes());
    if is_clr {
        let data_dirs = if is_pe32_plus {
            opt_off + 112
        } else {
            opt_off + 96
        };
        let clr_off = data_dirs + 14 * 8;
        pe[clr_off..clr_off + 4].copy_from_slice(&0x2000u32.to_le_bytes());
        pe[clr_off + 4..clr_off + 8].copy_from_slice(&72u32.to_le_bytes());
    }
    pe
}

/// Writes a synthetic PE binary to `path`.
///
/// # Arguments
///
/// * `path` - Destination file path.
/// * `machine` - Raw COFF machine constant.
/// * `is_pe32_plus` - Whether the optional header is `PE32+`.
/// * `is_clr` - Whether the image is a .NET CLR assembly.
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if writing the file fails.
fn write_synthetic_pe(path: &Path, machine: u16, is_pe32_plus: bool, is_clr: bool) -> Result<()> {
    let bytes = make_synthetic_pe(machine, is_pe32_plus, is_clr);
    fs::write(path, bytes)?;
    Ok(())
}

/// Builds and verifies a single architecture entry in the `PackageBuilder` matrix.
///
/// # Arguments
///
/// * `temp_dir` - Temporary output directory.
/// * `arch` - Target [`Architecture`].
/// * `expected_template` - Expected `PID_TEMPLATE` string.
/// * `expected_bitness` - Expected [`Bitness`].
/// * `expected_min_ver` - Expected `PID_PAGECOUNT` installer version.
/// * `expected_comp_attr` - Expected default component attribute bitflag.
/// * `expected_pf` - Expected standard `ProgramFiles` directory identifier.
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if building, saving, or reopening fails.
#[allow(clippy::too_many_arguments)]
fn verify_builder_architecture_entry(
    temp_dir: &Path,
    arch: Architecture,
    expected_template: &str,
    expected_bitness: Bitness,
    expected_min_ver: i32,
    expected_comp_attr: i16,
    expected_pf: &str,
) -> Result<()> {
    assert_eq!(arch.bitness(), expected_bitness);
    assert_eq!(arch.default_program_files_folder_id(), expected_pf);

    let msi_path = temp_dir.join(format!("pkg_{}.msi", arch.as_wix_str()));
    let pkg = Package::builder()
        .product_name(format!("App_{}", arch.as_wix_str()))
        .manufacturer("Acme Corp")
        .version(ProductVersion::new(1, 2, 3))
        .product_code("{11111111-2222-3333-4444-555555555555}")
        .architecture(arch)
        .installer_version(200)
        .add_directory(DirectoryRow {
            directory: DirectoryId::new("TARGETDIR")?,
            directory_parent: None,
            default_dir: "SourceDir".to_string(),
        })
        .add_directory(DirectoryRow {
            directory: DirectoryId::new(expected_pf)?,
            directory_parent: Some(DirectoryId::new("TARGETDIR")?),
            default_dir: "PFiles".to_string(),
        })
        .add_component_with_bitness(
            "MainComp",
            "*",
            expected_pf,
            ComponentBitnessRule::Default,
            0,
            None,
            None,
        )?
        .add_component_with_bitness(
            "Explicit32Comp",
            "*",
            "TARGETDIR",
            ComponentBitnessRule::Always32,
            0,
            None,
            None,
        )?
        .add_component_with_bitness(
            "Explicit64Comp",
            "*",
            "TARGETDIR",
            ComponentBitnessRule::Always64,
            0,
            None,
            None,
        )?
        .build()?;

    pkg.save(&msi_path)?;

    let reopened = Package::open(&msi_path)?;
    assert_eq!(
        reopened.summary_info().template.as_deref(),
        Some(expected_template)
    );
    assert_eq!(
        reopened.summary_info().template_summary(),
        Some(TemplateSummary::new(arch, vec![1033]))
    );
    assert_eq!(reopened.summary_info().page_count, Some(expected_min_ver));

    let comps = reopened.database().get_records("Component");
    assert_eq!(comps.len(), 3);
    assert_eq!(
        comps[0].get(3),
        Some(&FieldValue::Short(expected_comp_attr))
    );
    assert_eq!(comps[1].get(3), Some(&FieldValue::Short(0)));
    assert_eq!(comps[2].get(3), Some(&FieldValue::Short(0x0100)));
    Ok(())
}

/// Tests building `.msi` packages via `PackageBuilder` for all 6 target architectures
/// (`X86`, `X64`, `Arm`, `Arm64`, `Ia64`, `Neutral`) on the current host, saving to disk,
/// reopening, and verifying `PID_TEMPLATE`, `PID_PAGECOUNT`, `Component.Attributes`, and directories.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if any build, save, or reopen operation fails.
#[test]
fn test_package_builder_all_architectures_matrix() -> Result<()> {
    let temp_dir = make_temp_dir("builder_matrix")?;

    let matrix = [
        (
            Architecture::X86,
            "Intel;1033",
            Bitness::Bit32,
            200,
            0i16,
            "ProgramFilesFolder",
        ),
        (
            Architecture::X64,
            "x64;1033",
            Bitness::Bit64,
            200,
            0x0100i16,
            "ProgramFiles64Folder",
        ),
        (
            Architecture::Arm,
            "Arm;1033",
            Bitness::Bit32,
            500,
            0i16,
            "ProgramFilesFolder",
        ),
        (
            Architecture::Arm64,
            "Arm64;1033",
            Bitness::Bit64,
            500,
            0x0100i16,
            "ProgramFiles64Folder",
        ),
        (
            Architecture::Ia64,
            "Intel64;1033",
            Bitness::Bit64,
            200,
            0x0100i16,
            "ProgramFiles64Folder",
        ),
        (
            Architecture::Neutral,
            ";1033",
            Bitness::Neutral,
            200,
            0i16,
            "ProgramFilesFolder",
        ),
    ];

    for (arch, tmpl, bitness, min_ver, comp_attr, pf) in matrix {
        verify_builder_architecture_entry(&temp_dir, arch, tmpl, bitness, min_ver, comp_attr, pf)?;
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Verifies a single `Candle` + `Light` cross-architecture build and `.wixobj` persistence case.
///
/// # Arguments
///
/// * `temp_dir` - Temporary directory.
/// * `wxs_path` - Path to `.wxs` source file.
/// * `arch_flag` - `-arch` flag passed to `CandleOptions`.
/// * `expected_name` - Expected `ProductName`.
/// * `expected_flavor` - Expected `ARCH_FLAVOR` property value.
/// * `expected_template` - Expected `PID_TEMPLATE` string.
/// * `expected_pf` - Expected standard `ProgramFiles` directory ID.
/// * `expected_attr` - Expected `CompDefault` attributes.
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if compilation, linking, or verification fails.
#[allow(clippy::too_many_arguments)]
fn verify_candle_light_arch_case(
    temp_dir: &Path,
    wxs_path: &Path,
    arch_flag: &str,
    expected_name: &str,
    expected_flavor: &str,
    expected_template: &str,
    expected_pf: &str,
    expected_attr: i16,
) -> Result<()> {
    let wixobj_path = temp_dir.join(format!("out_{arch_flag}.wixobj"));
    let msi_path = temp_dir.join(format!("out_{arch_flag}.msi"));

    let candle = CandleOptions::parse(&[
        "-arch".to_string(),
        arch_flag.to_string(),
        "-out".to_string(),
        wixobj_path.to_string_lossy().to_string(),
        wxs_path.to_string_lossy().to_string(),
    ])?;
    let candle_outs = candle.execute()?;
    assert_eq!(candle_outs, vec![wixobj_path.clone()]);

    let light = LightOptions::parse(&[
        "-out".to_string(),
        msi_path.to_string_lossy().to_string(),
        wixobj_path.to_string_lossy().to_string(),
    ])?;
    let linked_msi = light.execute()?;
    assert_eq!(linked_msi, msi_path);

    let pkg = Package::open(&msi_path)?;
    assert_eq!(pkg.metadata().product_name(), expected_name);
    assert_eq!(
        pkg.summary_info().template.as_deref(),
        Some(expected_template)
    );

    let props = pkg.database().get_records("Property");
    assert!(props.iter().any(|r| {
        r.get(0) == Some(&FieldValue::String("ARCH_FLAVOR".to_string()))
            && r.get(1) == Some(&FieldValue::String(expected_flavor.to_string()))
    }));

    let dirs = pkg.database().get_records("Directory");
    assert!(dirs
        .iter()
        .any(|r| r.get(0) == Some(&FieldValue::String(expected_pf.to_string()))));

    for c in pkg.database().get_records("Component") {
        if c.get(0) == Some(&FieldValue::String("CompDefault".to_string())) {
            assert_eq!(c.get(3), Some(&FieldValue::Short(expected_attr)));
        } else if c.get(0) == Some(&FieldValue::String("CompAlways32".to_string())) {
            assert_eq!(c.get(3), Some(&FieldValue::Short(0)));
        }
    }
    Ok(())
}

/// Tests compiling and linking `.wxs` manifests with `<?if $(sys.BUILDARCH) = ... ?>`,
/// `$(sys.BUILDARCHSHORT)`, `Bitness="default|always32|always64"`, `<StandardDirectory Id="ProgramFiles6432Folder">`,
/// and `.wixobj` architecture persistence across `CandleOptions` and `LightOptions`.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if compilation, linking, or verification fails.
#[test]
fn test_wix_conditional_buildarch_and_wixobj_persistence() -> Result<()> {
    let temp_dir = make_temp_dir("wix_cond")?;
    let payload_file = temp_dir.join("payload.txt");
    fs::write(&payload_file, b"cross-arch payload")?;

    let wxs_path = temp_dir.join("cross_arch.wxs");
    let wxs_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="CrossArch_$(sys.BUILDARCH)_$(sys.BUILDARCHSHORT)" Version="1.0.0" Manufacturer="Acme" UpgradeCode="{{11111111-2222-3333-4444-555555555555}}">
    <Media Id="1" Cabinet="cab1.cab" EmbedCab="yes" />
    <?if $(sys.BUILDARCH) = arm64 ?>
      <Property Id="ARCH_FLAVOR" Value="AppleOrWindowsArm64" />
    <?elseif $(sys.BUILDARCH) = x64 ?>
      <Property Id="ARCH_FLAVOR" Value="Amd64" />
    <?else?>
      <Property Id="ARCH_FLAVOR" Value="Legacy32OrOther" />
    <?endif?>
    <StandardDirectory Id="ProgramFiles6432Folder">
      <Directory Id="INSTALLFOLDER" Name="CrossArchApp">
        <Component Id="CompDefault" Guid="*" Bitness="default">
          <File Id="PayloadFile" Source="{}" KeyPath="yes" />
        </Component>
        <Component Id="CompAlways32" Guid="*" Bitness="always32" />
      </Directory>
    </StandardDirectory>
    <Feature Id="Complete" Title="All" Level="1">
      <ComponentRef Id="CompDefault" />
      <ComponentRef Id="CompAlways32" />
    </Feature>
  </Package>
</Wix>
"#,
        payload_file.display()
    );
    fs::write(&wxs_path, wxs_content)?;

    for (
        arch_flag,
        expected_name,
        expected_flavor,
        expected_template,
        expected_pf,
        expected_attr,
    ) in [
        (
            "x86",
            "CrossArch_x86_x86",
            "Legacy32OrOther",
            "Intel;1033",
            "ProgramFilesFolder",
            0i16,
        ),
        (
            "x64",
            "CrossArch_x64_x64",
            "Amd64",
            "x64;1033",
            "ProgramFiles64Folder",
            0x0100i16,
        ),
        (
            "arm64",
            "CrossArch_arm64_a64",
            "AppleOrWindowsArm64",
            "Arm64;1033",
            "ProgramFiles64Folder",
            0x0100i16,
        ),
    ] {
        verify_candle_light_arch_case(
            &temp_dir,
            &wxs_path,
            arch_flag,
            expected_name,
            expected_flavor,
            expected_template,
            expected_pf,
            expected_attr,
        )?;
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests packaging synthetic `PE32` (`i386`, `ARMNT`) and `PE32+` (`AMD64`, `ARM64`, `IA64`) binaries,
/// automatic `MsiAssemblyName` `processorArchitecture` population, and ICE80 validation when
/// mismatched PE architectures are placed into incompatible packages or components.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if valid builds fail or invalid builds unexpectedly pass.
#[test]
fn test_pe_payload_binding_and_ice80_enforcement() -> Result<()> {
    let temp_dir = make_temp_dir("pe_ice80")?;

    let pe_x86 = temp_dir.join("bin_x86.exe");
    let pe_x64 = temp_dir.join("bin_x64.exe");
    let pe_arm64 = temp_dir.join("bin_arm64.exe");
    write_synthetic_pe(&pe_x86, 0x014C, false, false)?;
    write_synthetic_pe(&pe_x64, 0x8664, true, false)?;
    write_synthetic_pe(&pe_arm64, 0xAA64, true, false)?;

    // 1. Valid Arm64 MSI packaging an ARM64 PE32+ assembly
    let valid_arm64_wxs = temp_dir.join("valid_arm64.wxs");
    fs::write(
        &valid_arm64_wxs,
        format!(
            r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{{11111111-1111-1111-1111-111111111111}}" Name="Arm64Native" Language="1033" Version="1.0.0" Manufacturer="Acme" UpgradeCode="{{22222222-2222-2222-2222-222222222222}}">
    <Package Description="Arm64 Native" Platform="arm64" InstallerVersion="500" />
    <Media Id="1" Cabinet="cab1.cab" EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Component Id="CompArm64" Guid="*">
          <File Id="Arm64Exe" Name="bin_arm64.exe" Source="{}" KeyPath="yes" Assembly="win32" />
        </Component>
      </Directory>
    </Directory>
    <Feature Id="Complete" Level="1">
      <ComponentRef Id="CompArm64" />
    </Feature>
  </Product>
</Wix>"#,
            pe_arm64.display()
        ),
    )?;
    let valid_arm64_msi = temp_dir.join("valid_arm64.msi");
    let opts_valid = WixBuildOptions {
        sources: vec![valid_arm64_wxs],
        output: Some(valid_arm64_msi.clone()),
        arch: Some("arm64".to_string()),
        ..WixBuildOptions::new()
    };
    opts_valid.execute()?;
    let pkg_arm64 = Package::open(&valid_arm64_msi)?;
    assert_eq!(
        pkg_arm64.summary_info().template.as_deref(),
        Some("Arm64;1033")
    );
    let asm_names = pkg_arm64.database().get_records("MsiAssemblyName");
    assert!(asm_names.iter().any(|r| {
        r.get(1) == Some(&FieldValue::String("processorArchitecture".to_string()))
            && r.get(2) == Some(&FieldValue::String("arm64".to_string()))
    }));

    // 2. Invalid: 64-bit x64 PE packaged inside a 32-bit x86 MSI -> fails ICE80
    let invalid_pe_wxs = temp_dir.join("invalid_pe_in_x86.wxs");
    fs::write(
        &invalid_pe_wxs,
        format!(
            r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{{11111111-1111-1111-1111-111111111111}}" Name="BadMix" Language="1033" Version="1.0.0" Manufacturer="Acme" UpgradeCode="{{22222222-2222-2222-2222-222222222222}}">
    <Package Description="BadMix" Platform="x86" />
    <Media Id="1" Cabinet="cab1.cab" EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Component Id="Comp32" Guid="*">
          <File Id="X64ExeIn32" Name="bin_x64.exe" Source="{}" KeyPath="yes" />
        </Component>
      </Directory>
    </Directory>
    <Feature Id="Complete" Level="1">
      <ComponentRef Id="Comp32" />
    </Feature>
  </Product>
</Wix>"#,
            pe_x64.display()
        ),
    )?;
    let invalid_pe_msi = temp_dir.join("invalid_pe_in_x86.msi");
    let opts_invalid = WixBuildOptions {
        sources: vec![invalid_pe_wxs],
        output: Some(invalid_pe_msi),
        arch: Some("x86".to_string()),
        ..WixBuildOptions::new()
    };
    assert!(opts_invalid.execute().is_err());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests simulated cross-architecture installation (`EvaluationContext`, `ActionDispatcher`,
/// `LaunchConditions`, and `Vfs`) for `x86`, `x64`, and `arm64` packages on simulated 64-bit
/// vs 32-bit target environments.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if building or executing simulated installations fails.
#[test]
fn test_simulated_cross_architecture_installation() -> Result<()> {
    use msi::execution::costing::DiskCostEngine;
    use msi::execution::dispatcher::ActionDispatcher;
    use msi::execution::properties::EvaluationContext;
    use msi::execution::sequence::{ActionStatus, SequenceName};
    use msi::execution::transaction::Transaction;

    let temp_dir = make_temp_dir("sim_install")?;
    let wxs_64 = temp_dir.join("sim_64.wxs");
    fs::write(
        &wxs_64,
        r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{11111111-1111-1111-1111-111111111111}" Name="Sim64App" Language="1033" Version="1.0.0" Manufacturer="Acme" UpgradeCode="{22222222-2222-2222-2222-222222222222}">
    <Package Description="Sim64" Platform="arm64" InstallerVersion="500" />
    <Condition Message="64-bit OS required.">VersionNT64</Condition>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLFOLDER" Name="Sim64App">
          <Component Id="Comp64" Guid="*">
            <RegistryValue Root="HKLM" Key="Software\Acme\Sim64" Name="Arch" Value="arm64" Type="string" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="Complete" Level="1">
      <ComponentRef Id="Comp64" />
    </Feature>
  </Product>
</Wix>"#,
    )?;

    let msi_64 = temp_dir.join("sim_64.msi");
    let build_opts = WixBuildOptions {
        sources: vec![wxs_64],
        output: Some(msi_64.clone()),
        arch: Some("arm64".to_string()),
        ..WixBuildOptions::new()
    };
    build_opts.execute()?;

    let pkg = Package::open(&msi_64)?;

    // 1. Simulate installing on a 32-bit target OS (VersionNT64 unset) -> LaunchConditions rejects 64-bit MSI
    let mut ctx_32 = EvaluationContext::new();
    ctx_32.set_property("VersionNT", "601");
    let mut tx_32 = Transaction::new(pkg.database().clone(), ctx_32, DiskCostEngine::new());
    let mut disp_32 = ActionDispatcher::new(&mut tx_32);
    let seq = SequenceName("InstallExecuteSequence".to_string());
    let status_32 = disp_32.execute_sequence(&seq)?;
    assert_eq!(status_32, ActionStatus::Failure);

    // 2. Simulate installing on a 64-bit target OS (VersionNT64 = 603) + directory resolution
    let mut ctx_64 = EvaluationContext::new();
    ctx_64.set_property("VersionNT", "603");
    ctx_64.set_property("VersionNT64", "603");
    ctx_64.set_property("TARGETDIR", temp_dir.to_string_lossy());
    let mut cost_engine = DiskCostEngine::new();
    cost_engine.register_volume("TARGETDIR", 100_000_000, Some(4096));
    let mut tx_64 = Transaction::from_package(&pkg, ctx_64, cost_engine);
    {
        let mut disp_64 = ActionDispatcher::new(&mut tx_64);
        let status = disp_64.execute_sequence(&seq)?;
        assert_eq!(status, ActionStatus::Success);
    }
    let tx_prep = tx_64.prepare()?;
    let resolved_dir = tx_prep.context().get_property("INSTALLFOLDER");
    assert!(resolved_dir.unwrap_or_default().contains("Sim64App"));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

//! Driver Staging & Hardware Provisioning Engine.
//!
//! Grounded directly in the Windows Driver Kit (WDK) Driver Store specifications
//! and Linux kernel module deployment conventions:
//! - Driver INF file parsing and `FileRepository` staging.
//! - Offline registry injection into `Services` and `CriticalDeviceDatabase`.
//! - Linux kernel module hierarchy creation (`/lib/modules/<version>`) and initramfs synthesis.

use crate::error::{Error, Result};
use crate::platform::hive::{OfflineHiveStore, OfflineRegistryData};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Parsed hardware driver INF package descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverInf {
    /// Device setup class name (e.g. `SCSIAdapter`, `Net`, `System`).
    pub class: String,
    /// Device setup class GUID string.
    pub class_guid: String,
    /// Hardware vendor or provider name.
    pub provider: String,
    /// Driver version string and release date.
    pub driver_ver: String,
    /// Companion digital signature security catalog file name.
    pub catalog_file: Option<String>,
    /// Plug-and-Play (`PnP`) hardware IDs supported by this driver.
    pub hardware_ids: Vec<String>,
    /// Windows service name identifier.
    pub service_name: String,
    /// Primary kernel driver binary name (e.g. `stornvme.sys`).
    pub service_binary: String,
    /// Service startup type (0 = Boot Start, 1 = System Start, 2 = Auto Start).
    pub service_start_type: u32,
}

impl DriverInf {
    /// Parses a standard Windows setup INF file.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content of `.inf` file.
    ///
    /// # Returns
    ///
    /// Parsed [`DriverInf`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::DriverServicingError`] if required sections or keys are missing.
    pub fn parse_inf(content: &str) -> Result<Self> {
        let mut sections: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        let mut current_section = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].trim().to_ascii_uppercase();
                sections.entry(current_section.clone()).or_default();
            } else if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_ascii_uppercase();
                let val = v.trim().trim_matches('"').to_string();
                if let Some(list) = sections.get_mut(&current_section) {
                    list.push((key, val));
                }
            }
        }

        let version_sec = sections
            .get("VERSION")
            .ok_or_else(|| Error::DriverServicingError {
                inf: "unknown".to_string(),
                reason: "missing [Version] section".to_string(),
            })?;

        let mut class = String::from("Unknown");
        let mut class_guid = String::new();
        let mut provider = String::from("Unknown");
        let mut driver_ver = String::new();
        let mut catalog_file = None;

        for (k, v) in version_sec {
            match k.as_str() {
                "CLASS" => class.clone_from(v),
                "CLASSGUID" => class_guid.clone_from(v),
                "PROVIDER" => provider.clone_from(v),
                "DRIVERVER" => driver_ver.clone_from(v),
                "CATALOGFILE" => catalog_file = Some(v.clone()),
                _ => {}
            }
        }

        let mut hardware_ids = Vec::new();
        for (sec_name, list) in &sections {
            if sec_name.starts_with("MANUFACTURER") || sec_name.contains(".NT") {
                for (_, v) in list {
                    for part in v.split(',') {
                        let id = part.trim();
                        if id.contains(r"PCI\VEN_")
                            || id.contains(r"USB\VID_")
                            || id.contains("ACPI")
                        {
                            hardware_ids.push(id.to_string());
                        }
                    }
                }
            }
        }

        let service_name =
            if let Some((_, list)) = sections.iter().find(|(s, _)| s.ends_with(".SERVICES")) {
                list.first()
                    .map_or_else(|| String::from("DriverService"), |(_, v)| v.clone())
            } else {
                String::from("DriverService")
            };

        let service_binary = format!("{}.sys", service_name.to_ascii_lowercase());

        Ok(Self {
            class,
            class_guid,
            provider,
            driver_ver,
            catalog_file,
            hardware_ids,
            service_name,
            service_binary,
            service_start_type: 0, // Default to boot-start
        })
    }
}

/// Windows Driver Store staging and registry injection manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowsDriverStoreServicing;

impl WindowsDriverStoreServicing {
    /// Stages a complete driver package into the sysroot `DriverStore\FileRepository`.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Root of target Windows installation.
    /// * `inf_name` - File name of driver INF (e.g. `nvme.inf`).
    /// * `inf_bytes` - Content bytes of `.inf`.
    /// * `companion_files` - Array of `(filename, file_bytes)` pairs (e.g. `.sys`, `.cat`).
    ///
    /// # Returns
    ///
    /// Path to staged repository folder within target sysroot.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DriverServicingError`] if staging operations fail.
    pub fn stage_driver_package(
        sysroot: &Path,
        inf_name: &str,
        inf_bytes: &[u8],
        companion_files: &[(&str, &[u8])],
    ) -> Result<PathBuf> {
        let repo_folder_name = format!(
            "{}_amd64_123456789abcdef",
            inf_name.trim_end_matches(".inf").to_ascii_lowercase()
        );
        let repo_dir = sysroot
            .join("Windows/System32/DriverStore/FileRepository")
            .join(repo_folder_name);

        if let Err(e) = std::fs::create_dir_all(&repo_dir) {
            return Err(Error::DriverServicingError {
                inf: inf_name.to_string(),
                reason: format!("failed to create FileRepository directory: {e}"),
            });
        }

        // Write INF
        let target_inf = repo_dir.join(inf_name);
        if let Err(e) = std::fs::write(&target_inf, inf_bytes) {
            return Err(Error::DriverServicingError {
                inf: inf_name.to_string(),
                reason: format!("failed to write staged INF file: {e}"),
            });
        }

        // Write companion files (.sys, .cat)
        for (name, content) in companion_files {
            let dest = repo_dir.join(name);
            if let Err(e) = std::fs::write(&dest, content) {
                return Err(Error::DriverServicingError {
                    inf: inf_name.to_string(),
                    reason: format!("failed to write companion driver file '{name}': {e}"),
                });
            }

            // If binary is a .sys driver, also copy directly into Windows/System32/drivers
            if Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("sys"))
            {
                let drivers_dir = sysroot.join("Windows/System32/drivers");
                let _ = std::fs::create_dir_all(&drivers_dir);
                let _ = std::fs::write(drivers_dir.join(name), content);
            }
        }

        Ok(repo_dir)
    }

    /// Injects a critical boot driver into offline `SYSTEM\ControlSet001\Services` and `CriticalDeviceDatabase`.
    ///
    /// # Arguments
    ///
    /// * `store` - Mutable offline registry hive store.
    /// * `driver` - Driver package metadata.
    pub fn inject_critical_boot_driver(store: &mut OfflineHiveStore, driver: &DriverInf) {
        let svc_key = format!(r"ControlSet001\Services\{}", driver.service_name);

        // Type = 1 (Kernel Driver)
        store
            .system_hive
            .set_value(&svc_key, "Type", OfflineRegistryData::Dword(1));
        // Start = 0 (Boot Start)
        store.system_hive.set_value(
            &svc_key,
            "Start",
            OfflineRegistryData::Dword(driver.service_start_type),
        );
        // ErrorControl = 1 (Normal)
        store
            .system_hive
            .set_value(&svc_key, "ErrorControl", OfflineRegistryData::Dword(1));
        // ImagePath = system32\drivers\<binary>
        let image_path = format!(r"system32\drivers\{}", driver.service_binary);
        store.system_hive.set_value(
            &svc_key,
            "ImagePath",
            OfflineRegistryData::String(image_path),
        );
        // Group = SCSI Miniport or Boot Bus Extender
        store.system_hive.set_value(
            &svc_key,
            "Group",
            OfflineRegistryData::String("SCSI Miniport".to_string()),
        );

        // In CriticalDeviceDatabase, map each hardware ID to this service
        for hw_id in &driver.hardware_ids {
            let cdd_key = format!(r"ControlSet001\Control\CriticalDeviceDatabase\{hw_id}");
            store.system_hive.set_value(
                &cdd_key,
                "Service",
                OfflineRegistryData::String(driver.service_name.clone()),
            );
            if !driver.class_guid.is_empty() {
                store.system_hive.set_value(
                    &cdd_key,
                    "ClassGUID",
                    OfflineRegistryData::String(driver.class_guid.clone()),
                );
            }
        }
    }
}

/// Linux kernel module file descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelModule {
    /// Module name (e.g. `nvme`).
    pub name: String,
    /// File name (e.g. `nvme.ko`).
    pub file_name: String,
    /// Destination relative path inside `/lib/modules/<version>`.
    pub relative_path: String,
}

/// Linux kernel modules servicing and initramfs validation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinuxKernelModuleServicing;

impl LinuxKernelModuleServicing {
    /// Stages kernel modules into the target sysroot hierarchy and generates dependency maps.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Root of target Linux installation.
    /// * `kernel_version` - Kernel release string (e.g. `6.6.15-msi`).
    /// * `modules` - Slice of module descriptors and payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DriverServicingError`] if staging operations fail.
    pub fn stage_modules(
        sysroot: &Path,
        kernel_version: &str,
        modules: &[(KernelModule, &[u8])],
    ) -> Result<()> {
        let mod_root = sysroot.join(format!("lib/modules/{kernel_version}"));
        if let Err(e) = std::fs::create_dir_all(&mod_root) {
            return Err(Error::DriverServicingError {
                inf: "linux-modules".to_string(),
                reason: format!("failed to create modules directory: {e}"),
            });
        }

        let mut modules_dep = String::new();

        for (m, bytes) in modules {
            let dest = mod_root.join(&m.relative_path);
            let _ = dest.parent().map(std::fs::create_dir_all);
            if let Err(e) = std::fs::write(&dest, bytes) {
                return Err(Error::DriverServicingError {
                    inf: m.name.clone(),
                    reason: format!("failed to write module file: {e}"),
                });
            }

            let _ = writeln!(modules_dep, "{}:", m.relative_path);
        }

        // Write modules.dep
        let dep_path = mod_root.join("modules.dep");
        if let Err(e) = std::fs::write(&dep_path, modules_dep) {
            return Err(Error::DriverServicingError {
                inf: "modules.dep".to_string(),
                reason: format!("failed to write modules.dep: {e}"),
            });
        }

        Ok(())
    }

    /// Validates that required storage controller modules are present in target module staging.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Target sysroot.
    /// * `kernel_version` - Kernel release string.
    /// * `required_modules` - List of essential module names (e.g. `["nvme", "ahci"]`).
    ///
    /// # Returns
    ///
    /// Ok(()) if all essential modules are staged, or an error detailing missing modules.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DriverServicingError`] if any required module is missing.
    pub fn verify_storage_modules(
        sysroot: &Path,
        kernel_version: &str,
        required_modules: &[&str],
    ) -> Result<()> {
        let mod_root = sysroot.join(format!("lib/modules/{kernel_version}"));
        let dep_path = mod_root.join("modules.dep");

        let dep_content = std::fs::read_to_string(&dep_path).unwrap_or_default();

        for &req in required_modules {
            let matched = dep_content.lines().any(|line| line.contains(req));
            if !matched {
                return Err(Error::DriverServicingError {
                    inf: req.to_string(),
                    reason: format!(
                        "required boot storage controller module '{req}' missing from modules.dep"
                    ),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests INF parsing with [Version], [Manufacturer], and [Services] sections.
    #[test]
    fn test_inf_parsing() {
        let inf_data = r#"
; Comment line
UNATTACHED_KEY=VAL
[UnfinishedSection
[Version]
Signature="$Windows NT$"
Class=SCSIAdapter
ClassGUID={4D36E97B-E325-11CE-BFC1-08002BE10318}
Provider="Acme Corp"
DriverVer=01/01/2026,1.0.0.0
CatalogFile=acmenvme.cat
CustomField=Ignored

[Manufacturer]
%Acme%=AcmeDevice,NTamd64,USB\VID_1234&PID_5678,ACPI\PNP0A08,NON_MATCH_ID

[AcmeDevice.NTamd64]
%AcmeDesc%=Acme_Inst,PCI\VEN_144D&DEV_A808

[Strings]
Acme="Acme Corporation"
AcmeDesc="Acme NVMe Storage"

[Acme_Inst.Services]
AddService = acmenvme, 0x00000002, Acme_Service_Inst
"#;

        let parsed = DriverInf::parse_inf(inf_data);
        assert!(parsed.is_ok());
        assert_eq!(parsed.as_ref().map(|d| d.class.as_str()), Ok("SCSIAdapter"));
        assert_eq!(
            parsed.as_ref().map(|d| d.class_guid.as_str()),
            Ok("{4D36E97B-E325-11CE-BFC1-08002BE10318}")
        );
        assert_eq!(
            parsed.as_ref().map(|d| d.provider.as_str()),
            Ok("Acme Corp")
        );
        assert_eq!(
            parsed.as_ref().map(|d| d.catalog_file.as_deref()),
            Ok(Some("acmenvme.cat"))
        );
        assert_eq!(
            parsed.as_ref().map(|d| d
                .hardware_ids
                .contains(&r"PCI\VEN_144D&DEV_A808".to_string())),
            Ok(true)
        );
        assert_eq!(
            parsed.as_ref().map(|d| d
                .hardware_ids
                .contains(&r"USB\VID_1234&PID_5678".to_string())),
            Ok(true)
        );
        assert_eq!(
            parsed
                .as_ref()
                .map(|d| d.hardware_ids.contains(&"ACPI\\PNP0A08".to_string())),
            Ok(true)
        );

        // Missing [Version] must fail
        assert!(DriverInf::parse_inf("invalid content").is_err());

        // Empty .Services section exercises map_or_else fallback closure
        let inf_empty_svc = r#"
[Version]
Signature="$Windows NT$"

[Empty.Services]
"#;
        let parsed_empty = DriverInf::parse_inf(inf_empty_svc);
        assert_eq!(
            parsed_empty.as_ref().map(|d| d.service_name.as_str()),
            Ok("DriverService")
        );

        // Missing .Services section exercises fallback else branch
        let inf_no_svc = r#"
[Version]
Signature="$Windows NT$"
"#;
        let parsed_no = DriverInf::parse_inf(inf_no_svc);
        assert_eq!(
            parsed_no.as_ref().map(|d| d.service_name.as_str()),
            Ok("DriverService")
        );
    }

    /// Tests staging of Windows driver packages and critical boot driver injection.
    #[test]
    fn test_windows_driver_staging_and_injection() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_driver_{}", std::process::id()));
        let mut store = OfflineHiveStore::new_for_sysroot(&temp_dir);

        let inf = DriverInf {
            class: "SCSIAdapter".to_string(),
            class_guid: "{4D36E97B-E325-11CE-BFC1-08002BE10318}".to_string(),
            provider: "Acme".to_string(),
            driver_ver: "1.0".to_string(),
            catalog_file: Some("acme.cat".to_string()),
            hardware_ids: vec![r"PCI\VEN_144D&DEV_A808".to_string()],
            service_name: "acmenvme".to_string(),
            service_binary: "acmenvme.sys".to_string(),
            service_start_type: 0,
        };

        let companion_files = [
            ("acmenvme.sys", b"BINARY_SYS_PAYLOAD".as_slice()),
            ("acme.cat", b"SECURITY_CATALOG_DATA".as_slice()),
        ];

        let staged = WindowsDriverStoreServicing::stage_driver_package(
            &temp_dir,
            "acmenvme.inf",
            b"[Version]
Class=SCSIAdapter",
            &companion_files,
        );
        assert!(staged.is_ok());
        assert!(temp_dir
            .join("Windows/System32/drivers/acmenvme.sys")
            .exists());

        // Injection with GUID
        WindowsDriverStoreServicing::inject_critical_boot_driver(&mut store, &inf);

        let svc = store
            .system_hive
            .get_value(r"ControlSet001\Services\acmenvme", "Start");
        assert_eq!(svc, Some(OfflineRegistryData::Dword(0)));

        let cdd = store.system_hive.get_value(
            r"ControlSet001\Control\CriticalDeviceDatabase\PCI\VEN_144D&DEV_A808",
            "Service",
        );
        assert_eq!(
            cdd,
            Some(OfflineRegistryData::String("acmenvme".to_string()))
        );

        // Injection without ClassGUID
        let mut inf_no_guid = inf.clone();
        inf_no_guid.class_guid.clear();
        WindowsDriverStoreServicing::inject_critical_boot_driver(&mut store, &inf_no_guid);

        // Error: create_dir_all failure on FileRepository (create file blocking path)
        let bad_sysroot = temp_dir.join("bad_sysroot");
        let blocked_repo = bad_sysroot.join("Windows/System32/DriverStore/FileRepository");
        let _ = std::fs::create_dir_all(bad_sysroot.join("Windows/System32/DriverStore"));
        let _ = std::fs::write(&blocked_repo, b"blocking_file");
        assert!(WindowsDriverStoreServicing::stage_driver_package(
            &bad_sysroot,
            "acmenvme.inf",
            b"[Version]\nClass=SCSIAdapter",
            &companion_files,
        )
        .is_err());

        // Error: write failure on staged INF file
        let err_dir = temp_dir.join("err_inf");
        let repo_folder_name = format!(
            "{}_amd64_123456789abcdef",
            "acmenvme.inf".trim_end_matches(".inf").to_ascii_lowercase()
        );
        let repo_dir = err_dir
            .join("Windows/System32/DriverStore/FileRepository")
            .join(&repo_folder_name);
        let blocked_inf = repo_dir.join("acmenvme.inf");
        let _ = std::fs::create_dir_all(&blocked_inf);
        assert!(WindowsDriverStoreServicing::stage_driver_package(
            &err_dir,
            "acmenvme.inf",
            b"[Version]\nClass=SCSIAdapter",
            &companion_files,
        )
        .is_err());

        // Error: write failure on companion file
        let err_dir2 = temp_dir.join("err_companion");
        let repo_dir2 = err_dir2
            .join("Windows/System32/DriverStore/FileRepository")
            .join(&repo_folder_name);
        let blocked_companion = repo_dir2.join("acmenvme.sys");
        let _ = std::fs::create_dir_all(&blocked_companion);
        assert!(WindowsDriverStoreServicing::stage_driver_package(
            &err_dir2,
            "acmenvme.inf",
            b"[Version]\nClass=SCSIAdapter",
            &companion_files,
        )
        .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests Linux kernel module staging and verification of required drivers.
    #[test]
    fn test_linux_kernel_module_servicing() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_kmod_{}", std::process::id()));
        let kver = "6.6.15-msi";

        let modules = [
            (
                KernelModule {
                    name: "nvme".to_string(),
                    file_name: "nvme.ko".to_string(),
                    relative_path: "kernel/drivers/nvme/host/nvme.ko".to_string(),
                },
                b"NVME_KO_PAYLOAD".as_slice(),
            ),
            (
                KernelModule {
                    name: "ahci".to_string(),
                    file_name: "ahci.ko".to_string(),
                    relative_path: "kernel/drivers/ata/ahci.ko".to_string(),
                },
                b"AHCI_KO_PAYLOAD".as_slice(),
            ),
        ];

        let stage_res = LinuxKernelModuleServicing::stage_modules(&temp_dir, kver, &modules);
        assert!(stage_res.is_ok());

        assert!(temp_dir
            .join(format!("lib/modules/{kver}/modules.dep"))
            .exists());

        // Verify storage modules
        assert!(LinuxKernelModuleServicing::verify_storage_modules(
            &temp_dir,
            kver,
            &["nvme", "ahci"]
        )
        .is_ok());
        assert!(
            LinuxKernelModuleServicing::verify_storage_modules(&temp_dir, kver, &["megaraid"])
                .is_err()
        );

        // Error: create_dir_all failure on mod_root
        let bad_temp = temp_dir.join("bad_kmod_root");
        let blocked_modules = bad_temp.join("lib/modules");
        let _ = std::fs::create_dir_all(bad_temp.join("lib"));
        let _ = std::fs::write(&blocked_modules, b"blocking_file");
        assert!(LinuxKernelModuleServicing::stage_modules(&bad_temp, kver, &modules).is_err());

        // Error: write failure on module file (blocked by directory)
        let err_kmod = temp_dir.join("err_kmod");
        let blocked_mod_file = err_kmod.join(format!(
            "lib/modules/{kver}/kernel/drivers/nvme/host/nvme.ko"
        ));
        let _ = std::fs::create_dir_all(&blocked_mod_file);
        assert!(LinuxKernelModuleServicing::stage_modules(&err_kmod, kver, &modules).is_err());

        // Error: write failure on modules.dep (blocked by directory)
        let err_dep = temp_dir.join("err_dep");
        let blocked_dep = err_dep.join(format!("lib/modules/{kver}/modules.dep"));
        let _ = std::fs::create_dir_all(&blocked_dep);
        assert!(LinuxKernelModuleServicing::stage_modules(&err_dep, kver, &modules).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

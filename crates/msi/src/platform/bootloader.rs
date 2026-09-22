//! Bootloader & Firmware Provisioning Engine.
//!
//! Grounded directly in UEFI Specification Chapter 3 (Boot Manager) and Chapter 13 (Protocols):
//! - EFI System Partition (ESP) directory layout and canonical loader placement (`/EFI/BOOT/BOOTX64.EFI`).
//! - Offline Windows Boot Configuration Data (BCD) hive synthesis and `{bootmgr}` / `{default}` object wiring.
//! - Linux bootloader generators: `systemd-boot` entries, `GRUB2` `grub.cfg`, and `Limine` `limine.cfg`.
//! - EFI NVRAM Variable manipulation (`efivarfs`) for non-volatile `BootXXXX` and `BootOrder` entries.

use crate::error::{Error, Result};
use crate::platform::hive::{OfflineRegistryData, OfflineRegistryHive};
use crate::platform::partition::PartitionUuid;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Supported bootloader firmware payload architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BootloaderKind {
    /// Microsoft Windows Boot Manager (`bootmgfw.efi` + BCD).
    WindowsBootManager,
    /// Systemd Unified Kernel Image and simple loader (`systemd-bootx64.efi`).
    SystemdBoot,
    /// Grand Unified Bootloader version 2 (`grubx64.efi`).
    Grub2,
    /// Modern lightweight multi-protocol bootloader (`BOOTX64.EFI` / `limine.cfg`).
    Limine,
}

/// Staging manager for EFI System Partition (ESP) directory structures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EspLayoutManager;

impl EspLayoutManager {
    /// Stages the default fallback UEFI bootloader binary (`/EFI/BOOT/BOOTX64.EFI`).
    ///
    /// # Arguments
    ///
    /// * `esp_root` - Root mount path of the EFI System Partition.
    /// * `binary` - Compiled executable bytes of the UEFI bootloader.
    ///
    /// # Returns
    ///
    /// Destination path of the staged binary.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderError`] if staging operations fail.
    pub fn stage_fallback_bootloader(esp_root: &Path, binary: &[u8]) -> Result<PathBuf> {
        let boot_dir = esp_root.join("EFI/BOOT");
        if let Err(e) = std::fs::create_dir_all(&boot_dir) {
            return Err(Error::BootloaderError {
                target: "ESP/BOOT".to_string(),
                reason: format!("failed to create EFI/BOOT directory: {e}"),
            });
        }

        let target_path = boot_dir.join("BOOTX64.EFI");
        if let Err(e) = std::fs::write(&target_path, binary) {
            return Err(Error::BootloaderError {
                target: "BOOTX64.EFI".to_string(),
                reason: format!("failed to write fallback bootloader binary: {e}"),
            });
        }

        Ok(target_path)
    }

    /// Stages an OS vendor-specific UEFI bootloader binary (e.g. `/EFI/Microsoft/Boot/bootmgfw.efi`).
    ///
    /// # Arguments
    ///
    /// * `esp_root` - Root mount path of the EFI System Partition.
    /// * `vendor` - Vendor folder name (e.g. `Microsoft`, `systemd`, `grub`).
    /// * `filename` - Target loader file name (e.g. `bootmgfw.efi`).
    /// * `binary` - Compiled executable bytes.
    ///
    /// # Returns
    ///
    /// Destination path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderError`] if write fails.
    pub fn stage_vendor_bootloader(
        esp_root: &Path,
        vendor: &str,
        filename: &str,
        binary: &[u8],
    ) -> Result<PathBuf> {
        let vendor_dir = esp_root.join("EFI").join(vendor);
        if let Err(e) = std::fs::create_dir_all(&vendor_dir) {
            return Err(Error::BootloaderError {
                target: format!("EFI/{vendor}"),
                reason: format!("failed to create vendor directory: {e}"),
            });
        }

        let dest = vendor_dir.join(filename);
        if let Err(e) = std::fs::write(&dest, binary) {
            return Err(Error::BootloaderError {
                target: filename.to_string(),
                reason: format!("failed to write vendor bootloader: {e}"),
            });
        }

        Ok(dest)
    }
}

/// Windows Boot Configuration Data (BCD) store generator and manipulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowsBcdStore;

impl WindowsBcdStore {
    /// Well-known GUID for Windows Boot Manager object (`{bootmgr}`).
    pub const BOOTMGR_GUID: &'static str = "{9dea862c-5cdd-4e70-acc1-f32b344d4795}";
    /// Well-known GUID for default Windows OS Loader object (`{default}`).
    pub const DEFAULT_OS_GUID: &'static str = "{a5a30a10-d40a-4715-9cdf-2d2b32c6b78d}";

    /// Synthesizes a new, offline BCD registry hive at `<ESP>/EFI/Microsoft/Boot/BCD`.
    ///
    /// # Arguments
    ///
    /// * `esp_root` - Root of the EFI System Partition.
    /// * `os_partition_uuid` - Unique partition GUID of Windows OS partition.
    /// * `os_name` - Operating system display name (e.g. `Windows 11`).
    ///
    /// # Returns
    ///
    /// Path to the generated BCD hive file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderError`] if hive generation or disk write fails.
    pub fn create_offline_bcd(
        esp_root: &Path,
        os_partition_uuid: PartitionUuid,
        os_name: &str,
    ) -> Result<PathBuf> {
        let bcd_dir = esp_root.join("EFI/Microsoft/Boot");
        if let Err(e) = std::fs::create_dir_all(&bcd_dir) {
            return Err(Error::BootloaderError {
                target: "BCD".to_string(),
                reason: format!("failed to create BCD directory: {e}"),
            });
        }

        let mut hive = OfflineRegistryHive::new("BCD00000000");

        let bcd_key =
            |obj: &str, elem: &str| -> String { format!("Objects\\{obj}\\Elements\\{elem}") };

        // 1. Configure {bootmgr} object
        // Element 0x24000001: Description = "Windows Boot Manager"
        hive.set_value(
            &bcd_key(Self::BOOTMGR_GUID, "24000001"),
            "Element",
            OfflineRegistryData::String("Windows Boot Manager".to_string()),
        );
        // Element 0x23000003: DefaultObject = {default}
        hive.set_value(
            &bcd_key(Self::BOOTMGR_GUID, "23000003"),
            "Element",
            OfflineRegistryData::String(Self::DEFAULT_OS_GUID.to_string()),
        );
        // Element 0x24000004: DisplayOrder = [{default}]
        hive.set_value(
            &bcd_key(Self::BOOTMGR_GUID, "24000004"),
            "Element",
            OfflineRegistryData::MultiString(vec![Self::DEFAULT_OS_GUID.to_string()]),
        );
        // Element 0x25000004: Timeout = 30 seconds
        hive.set_value(
            &bcd_key(Self::BOOTMGR_GUID, "25000004"),
            "Element",
            OfflineRegistryData::Dword(30),
        );

        // 2. Configure {default} Windows OS Loader object
        // Element 0x12000004: Description = os_name
        hive.set_value(
            &bcd_key(Self::DEFAULT_OS_GUID, "12000004"),
            "Element",
            OfflineRegistryData::String(os_name.to_string()),
        );
        // Element 0x22000002: ApplicationPath = \Windows\system32\winload.efi
        hive.set_value(
            &bcd_key(Self::DEFAULT_OS_GUID, "22000002"),
            "Element",
            OfflineRegistryData::String(r"\Windows\system32\winload.efi".to_string()),
        );
        // Element 0x22000024: SystemRoot = \Windows
        hive.set_value(
            &bcd_key(Self::DEFAULT_OS_GUID, "22000024"),
            "Element",
            OfflineRegistryData::String(r"\Windows".to_string()),
        );
        // Element 0x25000025: NxPolicy = 1 (OptIn)
        hive.set_value(
            &bcd_key(Self::DEFAULT_OS_GUID, "25000025"),
            "Element",
            OfflineRegistryData::Dword(1),
        );
        // Element 0x21000001: Device = partition GUID
        hive.set_value(
            &bcd_key(Self::DEFAULT_OS_GUID, "21000001"),
            "Element",
            OfflineRegistryData::String(format!("{os_partition_uuid}")),
        );

        let bcd_path = bcd_dir.join("BCD");
        hive.save_to_file(&bcd_path)
            .map_err(|e| Error::BootloaderError {
                target: "BCD".to_string(),
                reason: format!("failed to write BCD hive: {e}"),
            })?;

        Ok(bcd_path)
    }
}

/// Linux bootloader configuration generators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxBootloaderConfig;

impl LinuxBootloaderConfig {
    /// Generates `systemd-boot` loader configuration files.
    ///
    /// # Arguments
    ///
    /// * `esp_root` - Root of the EFI System Partition.
    /// * `entry_name` - Loader entry identifier (e.g. `linux`).
    /// * `title` - Boot menu entry title.
    /// * `vmlinuz_path` - Path to kernel image relative to ESP root (e.g. `/vmlinuz-linux`).
    /// * `initrd_path` - Path to initramfs relative to ESP root (e.g. `/initramfs-linux.img`).
    /// * `cmdline_options` - Kernel arguments (e.g. `root=UUID=... rw quiet`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderError`] if writing configuration files fails.
    pub fn write_systemd_boot(
        esp_root: &Path,
        entry_name: &str,
        title: &str,
        vmlinuz_path: &str,
        initrd_path: &str,
        cmdline_options: &str,
    ) -> Result<()> {
        let entries_dir = esp_root.join("loader/entries");
        if let Err(e) = std::fs::create_dir_all(&entries_dir) {
            return Err(Error::BootloaderError {
                target: "systemd-boot".to_string(),
                reason: format!("failed to create loader/entries: {e}"),
            });
        }

        // 1. loader.conf
        let loader_conf = format!(
            "default {entry_name}.conf
timeout 5
console-mode max
"
        );
        let loader_path = esp_root.join("loader/loader.conf");
        let _ = std::fs::write(loader_path, loader_conf);

        // 2. entry.conf
        let mut entry_content = String::new();
        let _ = writeln!(entry_content, "title {title}");
        let _ = writeln!(entry_content, "linux {vmlinuz_path}");
        let _ = writeln!(entry_content, "initrd {initrd_path}");
        let _ = writeln!(entry_content, "options {cmdline_options}");

        let entry_path = entries_dir.join(format!("{entry_name}.conf"));
        std::fs::write(&entry_path, entry_content).map_err(|e| Error::BootloaderError {
            target: format!("{entry_name}.conf"),
            reason: format!("failed to write systemd-boot entry: {e}"),
        })?;

        Ok(())
    }

    /// Generates standard `GRUB2` configuration text (`grub.cfg`).
    ///
    /// # Arguments
    ///
    /// * `title` - Menu entry title.
    /// * `root_uuid` - Root partition UUID string.
    /// * `vmlinuz` - Kernel path.
    /// * `initrd` - Initramfs path.
    /// * `cmdline` - Additional kernel parameters.
    ///
    /// # Returns
    ///
    /// Generated `grub.cfg` text.
    #[must_use]
    pub fn generate_grub_cfg(
        title: &str,
        root_uuid: &str,
        vmlinuz: &str,
        initrd: &str,
        cmdline: &str,
    ) -> String {
        let mut s = String::from(
            "set timeout=5
",
        );
        s.push_str(
            "set default=0

",
        );
        let _ = writeln!(s, r#"menuentry "{title}" {{"#);
        s.push_str(
            "    insmod gzio
",
        );
        s.push_str(
            "    insmod part_gpt
",
        );
        s.push_str(
            "    insmod ext2
",
        );
        let _ = writeln!(s, "    search --no-floppy --fs-uuid --set=root {root_uuid}");
        let _ = writeln!(s, "    linux {vmlinuz} root=UUID={root_uuid} rw {cmdline}");
        let _ = writeln!(s, "    initrd {initrd}");
        s.push_str(
            "}
",
        );
        s
    }

    /// Generates lightweight `Limine` configuration text (`limine.cfg`).
    ///
    /// # Arguments
    ///
    /// * `title` - Menu entry title.
    /// * `cmdline` - Kernel command line string.
    ///
    /// # Returns
    ///
    /// Formatted `limine.cfg` string.
    #[must_use]
    pub fn generate_limine_cfg(title: &str, cmdline: &str) -> String {
        let mut s = String::from(
            "TIMEOUT=5

",
        );
        let _ = writeln!(s, ":{title}");
        s.push_str(
            "    PROTOCOL=linux
",
        );
        s.push_str(
            "    KERNEL_PATH=boot:///vmlinuz-linux
",
        );
        s.push_str(
            "    MODULE_PATH=boot:///initramfs-linux.img
",
        );
        let _ = writeln!(s, "    CMDLINE={cmdline}");
        s
    }
}

/// Manager for non-volatile EFI boot menu variables in `efivarfs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EfiNvramManager;

impl EfiNvramManager {
    /// EFI Global Variable UUID suffix.
    pub const EFI_GLOBAL_VARIABLE_GUID: &'static str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

    /// Creates a non-volatile UEFI boot menu entry (`BootXXXX`).
    ///
    /// # Arguments
    ///
    /// * `efivarfs_root` - Path to mounted `efivars` directory (defaults to `/sys/firmware/efi/efivars`).
    /// * `boot_index` - 4-digit hexadecimal boot index (e.g. `0x0001`).
    /// * `description` - Human-readable label displayed in BIOS/UEFI boot menu.
    /// * `partition_uuid` - Unique partition GUID of EFI System Partition.
    /// * `loader_path` - Path to EFI loader binary (e.g. `\EFI\BOOT\BOOTX64.EFI`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderError`] if `efivarfs` is inaccessible or variable writing fails.
    pub fn create_boot_entry(
        efivarfs_root: Option<&Path>,
        boot_index: u16,
        description: &str,
        partition_uuid: PartitionUuid,
        loader_path: &str,
    ) -> Result<()> {
        let root = efivarfs_root.unwrap_or_else(|| Path::new("/sys/firmware/efi/efivars"));
        if !root.exists() {
            return Err(Error::BootloaderError {
                target: "efivarfs".to_string(),
                reason: format!("efivarfs directory does not exist at {}", root.display()),
            });
        }

        let var_filename = format!("Boot{boot_index:04X}-{}", Self::EFI_GLOBAL_VARIABLE_GUID);
        let var_path = root.join(var_filename);

        // EFI Variable attributes header (4 bytes: Non-Volatile, BootService Access, Runtime Access)
        // Attribute flags: 0x00000007
        let mut payload = vec![0x07, 0x00, 0x00, 0x00];

        // EFI_LOAD_OPTION:
        // Attributes: u32 = 0x00000001 (LOAD_OPTION_ACTIVE)
        payload.extend_from_slice(&1u32.to_le_bytes());

        // FilePathListLength: u16 placeholder (updated later)
        let fpl_pos = payload.len();
        payload.extend_from_slice(&0u16.to_le_bytes());

        // Description: Null-terminated UTF-16 string
        for unit in description.encode_utf16() {
            payload.extend_from_slice(&unit.to_le_bytes());
        }
        payload.extend_from_slice(&0u16.to_le_bytes()); // Null terminator

        // FilePathList (Harddrive Device Path + File Path)
        let fp_start = payload.len();

        // Harddrive Media Device Path (Type 0x04, Subtype 0x01, Length 42 bytes)
        payload.push(0x04);
        payload.push(0x01);
        payload.extend_from_slice(&42u16.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes()); // Partition Number: 1
        payload.extend_from_slice(&2048u64.to_le_bytes()); // Partition Start: LBA 2048
        payload.extend_from_slice(&1_024_000_u64.to_le_bytes()); // Partition Size
        payload.extend_from_slice(&partition_uuid.0); // Partition Signature UUID
        payload.push(0x02); // Format: GUID Partition Table
        payload.push(0x02); // Signature Type: GUID

        // File Path Media Device Path (Type 0x04, Subtype 0x04, Length variable)
        let loader_utf16: Vec<u16> = loader_path.encode_utf16().collect();
        let file_path_len = 4 + (loader_utf16.len() * 2) + 2;
        let file_path_len_u16 = u16::try_from(file_path_len).unwrap_or(u16::MAX);
        payload.push(0x04);
        payload.push(0x04);
        payload.extend_from_slice(&file_path_len_u16.to_le_bytes());
        for u in loader_utf16 {
            payload.extend_from_slice(&u.to_le_bytes());
        }
        payload.extend_from_slice(&0u16.to_le_bytes());

        // End of Hardware Device Path (Type 0x7F, Subtype 0xFF, Length 4)
        payload.extend_from_slice(&[0x7F, 0xFF, 0x04, 0x00]);

        let fp_total_len =
            u16::try_from(payload.len().saturating_sub(fp_start)).unwrap_or(u16::MAX);
        payload[fpl_pos..fpl_pos + 2].copy_from_slice(&fp_total_len.to_le_bytes());

        std::fs::write(&var_path, payload).map_err(|e| Error::BootloaderError {
            target: format!("Boot{boot_index:04X}"),
            reason: format!("failed to write boot entry to efivarfs: {e}"),
        })?;

        Ok(())
    }

    /// Sets the active UEFI `BootOrder` list in `efivarfs`.
    ///
    /// # Arguments
    ///
    /// * `efivarfs_root` - Path to mounted `efivars` directory.
    /// * `order` - Slice of boot indices in desired priority order (e.g. `&[0x0001, 0x0000]`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderError`] if writing `BootOrder` fails.
    pub fn set_boot_order(efivarfs_root: Option<&Path>, order: &[u16]) -> Result<()> {
        let root = efivarfs_root.unwrap_or_else(|| Path::new("/sys/firmware/efi/efivars"));
        let var_filename = format!("BootOrder-{}", Self::EFI_GLOBAL_VARIABLE_GUID);
        let var_path = root.join(var_filename);

        // Attributes: 0x00000007 (Non-Volatile, BootService, Runtime)
        let mut payload = vec![0x07, 0x00, 0x00, 0x00];
        for idx in order {
            payload.extend_from_slice(&idx.to_le_bytes());
        }

        std::fs::write(&var_path, payload).map_err(|e| Error::BootloaderError {
            target: "BootOrder".to_string(),
            reason: format!("failed to write BootOrder variable: {e}"),
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests staging of fallback and vendor bootloader binaries.
    #[test]
    fn test_esp_staging() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_esp_{}", std::process::id()));
        let dummy_efi = vec![0x4D, 0x5A, 0x90, 0x00];

        let fallback = EspLayoutManager::stage_fallback_bootloader(&temp_dir, &dummy_efi);
        assert!(fallback.is_ok());
        assert!(temp_dir.join("EFI/BOOT/BOOTX64.EFI").exists());

        let vendor = EspLayoutManager::stage_vendor_bootloader(
            &temp_dir,
            "Microsoft",
            "bootmgfw.efi",
            &dummy_efi,
        );
        assert!(vendor.is_ok());
        assert!(temp_dir.join("EFI/Microsoft/bootmgfw.efi").exists());

        // Error on fallback create_dir_all: blocked by file
        let err_dir1 = temp_dir.join("err_fb_dir");
        let _ = std::fs::create_dir_all(err_dir1.join("EFI"));
        let _ = std::fs::write(err_dir1.join("EFI/BOOT"), b"blocking_file");
        assert!(EspLayoutManager::stage_fallback_bootloader(&err_dir1, &dummy_efi).is_err());

        // Error on fallback write: blocked by directory
        let err_dir2 = temp_dir.join("err_fb_write");
        let _ = std::fs::create_dir_all(err_dir2.join("EFI/BOOT/BOOTX64.EFI"));
        assert!(EspLayoutManager::stage_fallback_bootloader(&err_dir2, &dummy_efi).is_err());

        // Error on vendor create_dir_all: blocked by file
        let err_dir3 = temp_dir.join("err_vnd_dir");
        let _ = std::fs::create_dir_all(err_dir3.join("EFI"));
        let _ = std::fs::write(err_dir3.join("EFI/Microsoft"), b"blocking_file");
        assert!(EspLayoutManager::stage_vendor_bootloader(
            &err_dir3,
            "Microsoft",
            "bootmgfw.efi",
            &dummy_efi,
        )
        .is_err());

        // Error on vendor write: blocked by directory
        let err_dir4 = temp_dir.join("err_vnd_write");
        let _ = std::fs::create_dir_all(err_dir4.join("EFI/Microsoft/bootmgfw.efi"));
        assert!(EspLayoutManager::stage_vendor_bootloader(
            &err_dir4,
            "Microsoft",
            "bootmgfw.efi",
            &dummy_efi,
        )
        .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests Windows BCD store synthesis.
    #[test]
    fn test_windows_bcd_generation() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_bcd_{}", std::process::id()));
        let os_uuid = PartitionUuid([0x33; 16]);

        let bcd_res = WindowsBcdStore::create_offline_bcd(&temp_dir, os_uuid, "Windows 11 Pro");
        assert!(bcd_res.is_ok());
        assert!(temp_dir.join("EFI/Microsoft/Boot/BCD").exists());

        // Error on BCD create_dir_all: blocked by file
        let err_dir1 = temp_dir.join("err_bcd_dir");
        let _ = std::fs::create_dir_all(err_dir1.join("EFI/Microsoft"));
        let _ = std::fs::write(err_dir1.join("EFI/Microsoft/Boot"), b"blocking_file");
        assert!(WindowsBcdStore::create_offline_bcd(&err_dir1, os_uuid, "Windows 11 Pro").is_err());

        // Error on BCD write: blocked by directory
        let err_dir2 = temp_dir.join("err_bcd_write");
        let _ = std::fs::create_dir_all(err_dir2.join("EFI/Microsoft/Boot/BCD"));
        assert!(WindowsBcdStore::create_offline_bcd(&err_dir2, os_uuid, "Windows 11 Pro").is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests Linux bootloader configuration generators (systemd-boot, GRUB2, Limine).
    #[test]
    fn test_linux_bootloader_generators() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_linuxboot_{}", std::process::id()));

        // systemd-boot
        let sd_res = LinuxBootloaderConfig::write_systemd_boot(
            &temp_dir,
            "arch",
            "Arch Linux",
            "/vmlinuz-linux",
            "/initramfs-linux.img",
            "root=UUID=1234 rw",
        );
        assert!(sd_res.is_ok());
        assert!(temp_dir.join("loader/loader.conf").exists());
        assert!(temp_dir.join("loader/entries/arch.conf").exists());

        // Error on systemd-boot create_dir_all: blocked by file
        let err_dir1 = temp_dir.join("err_sd_dir");
        let _ = std::fs::create_dir_all(err_dir1.join("loader"));
        let _ = std::fs::write(err_dir1.join("loader/entries"), b"blocking_file");
        assert!(LinuxBootloaderConfig::write_systemd_boot(
            &err_dir1,
            "arch",
            "Arch Linux",
            "/vmlinuz-linux",
            "/initramfs-linux.img",
            "root=UUID=1234 rw",
        )
        .is_err());

        // Error on systemd-boot write: blocked by directory
        let err_dir2 = temp_dir.join("err_sd_write");
        let _ = std::fs::create_dir_all(err_dir2.join("loader/entries/arch.conf"));
        assert!(LinuxBootloaderConfig::write_systemd_boot(
            &err_dir2,
            "arch",
            "Arch Linux",
            "/vmlinuz-linux",
            "/initramfs-linux.img",
            "root=UUID=1234 rw",
        )
        .is_err());

        // GRUB2
        let grub = LinuxBootloaderConfig::generate_grub_cfg(
            "Ubuntu",
            "ABCD-1234",
            "/boot/vmlinuz",
            "/boot/initrd",
            "quiet splash",
        );
        assert!(grub.contains(r#"menuentry "Ubuntu""#));
        assert!(grub.contains("search --no-floppy --fs-uuid --set=root ABCD-1234"));

        // Limine
        let limine = LinuxBootloaderConfig::generate_limine_cfg("Debian", "root=UUID=1234 ro");
        assert!(limine.contains(":Debian"));
        assert!(limine.contains("PROTOCOL=linux"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests EFI NVRAM variable generation and `BootOrder` updates in mock efivarfs.
    #[test]
    fn test_efi_nvram_management() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_efivar_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let esp_uuid = PartitionUuid([0x44; 16]);
        let entry_res = EfiNvramManager::create_boot_entry(
            Some(&temp_dir),
            1,
            "msi-rs OS",
            esp_uuid,
            r"\EFI\BOOT\BOOTX64.EFI",
        );
        assert!(entry_res.is_ok());

        let expected_var = format!("Boot0001-{}", EfiNvramManager::EFI_GLOBAL_VARIABLE_GUID);
        assert!(temp_dir.join(&expected_var).exists());

        let order_res = EfiNvramManager::set_boot_order(Some(&temp_dir), &[1, 0]);
        assert!(order_res.is_ok());
        let expected_order = format!("BootOrder-{}", EfiNvramManager::EFI_GLOBAL_VARIABLE_GUID);
        assert!(temp_dir.join(&expected_order).exists());

        // Error on create_boot_entry write: blocked by directory
        let err_var = temp_dir.join(format!(
            "Boot0002-{}",
            EfiNvramManager::EFI_GLOBAL_VARIABLE_GUID
        ));
        let _ = std::fs::create_dir_all(&err_var);
        assert!(EfiNvramManager::create_boot_entry(
            Some(&temp_dir),
            2,
            "msi-rs OS",
            esp_uuid,
            r"\EFI\BOOT\BOOTX64.EFI",
        )
        .is_err());

        // Error on set_boot_order write: blocked by directory
        let err_dir = temp_dir.join("err_order_dir");
        let _ = std::fs::create_dir_all(&err_dir);
        let blocked_order = err_dir.join(format!(
            "BootOrder-{}",
            EfiNvramManager::EFI_GLOBAL_VARIABLE_GUID
        ));
        let _ = std::fs::create_dir_all(&blocked_order);
        assert!(EfiNvramManager::set_boot_order(Some(&err_dir), &[1, 0]).is_err());

        // Test None parameter for root path defaults
        let _ = EfiNvramManager::create_boot_entry(None, 1, "test", esp_uuid, "test");
        let _ = EfiNvramManager::set_boot_order(None, &[1, 0]);

        // Inaccessible efivars must fail
        let bad_dir = temp_dir.join("nonexistent");
        assert!(
            EfiNvramManager::create_boot_entry(Some(&bad_dir), 1, "test", esp_uuid, "test")
                .is_err()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

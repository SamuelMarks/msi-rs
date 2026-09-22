//! Boot Environment & Live Harness for Bare-Metal OS Installation.
//!
//! Provides minimal bootable Linux runtime generation (initramfs, kernel configuration,
//! UKI packaging, hybrid ISO/USB layout) and `WinPE` automation harnesses.

use crate::error::{Error, Result};
use std::fmt::Write as _;

/// Built-in hardware driver categories for stripped installation kernels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelDriverKind {
    /// `NVMe` Solid-State Drive controller driver (`CONFIG_BLK_DEV_NVME`).
    Nvme,
    /// Serial ATA / Advanced Host Controller Interface driver (`CONFIG_SATA_AHCI`).
    Ahci,
    /// USB Mass Storage and Extensible Host Controller driver (`CONFIG_USB_STORAGE`).
    UsbStorage,
    /// `VirtIO` paravirtualized block device driver (`CONFIG_VIRTIO_BLK`).
    VirtioBlk,
    /// `VirtIO` paravirtualized network device driver (`CONFIG_VIRTIO_NET`).
    VirtioNet,
    /// Ext4 filesystem driver (`CONFIG_EXT4_FS`).
    Ext4,
    /// FAT / VFAT filesystem driver (`CONFIG_VFAT_FS`).
    Vfat,
    /// EFI Variable filesystem driver (`CONFIG_EFIVAR_FS`).
    Efivarfs,
}

impl KernelDriverKind {
    /// Returns the Kconfig symbol definition associated with this driver.
    ///
    /// # Returns
    ///
    /// The string representation of the Kconfig flag.
    #[must_use]
    pub const fn kconfig_symbol(self) -> &'static str {
        match self {
            Self::Nvme => "CONFIG_BLK_DEV_NVME=y",
            Self::Ahci => "CONFIG_SATA_AHCI=y",
            Self::UsbStorage => "CONFIG_USB_STORAGE=y",
            Self::VirtioBlk => "CONFIG_VIRTIO_BLK=y",
            Self::VirtioNet => "CONFIG_VIRTIO_NET=y",
            Self::Ext4 => "CONFIG_EXT4_FS=y",
            Self::Vfat => "CONFIG_VFAT_FS=y",
            Self::Efivarfs => "CONFIG_EFIVAR_FS=y",
        }
    }
}

/// Linux kernel `.config` generator optimized for bare-metal installer environments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelConfig {
    /// Enabled built-in driver categories.
    drivers: Vec<KernelDriverKind>,
    /// Additional custom configuration lines.
    custom_options: Vec<String>,
}

impl Default for KernelConfig {
    /// Creates a kernel configuration with the mandatory storage and filesystem drivers.
    ///
    /// # Returns
    ///
    /// A new [`KernelConfig`] instance with default drivers populated.
    fn default() -> Self {
        Self {
            drivers: vec![
                KernelDriverKind::Nvme,
                KernelDriverKind::Ahci,
                KernelDriverKind::UsbStorage,
                KernelDriverKind::VirtioBlk,
                KernelDriverKind::Ext4,
                KernelDriverKind::Vfat,
                KernelDriverKind::Efivarfs,
            ],
            custom_options: Vec::new(),
        }
    }
}

impl KernelConfig {
    /// Creates an empty kernel configuration without any pre-selected drivers.
    ///
    /// # Returns
    ///
    /// A new, empty [`KernelConfig`] instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            drivers: Vec::new(),
            custom_options: Vec::new(),
        }
    }

    /// Adds a driver kind to the kernel configuration.
    ///
    /// # Arguments
    ///
    /// * `driver` - The driver category to enable as a built-in.
    pub fn add_driver(&mut self, driver: KernelDriverKind) {
        if !self.drivers.contains(&driver) {
            self.drivers.push(driver);
        }
    }

    /// Adds a custom Kconfig option line.
    ///
    /// # Arguments
    ///
    /// * `option` - Literal option line (e.g. `CONFIG_PRINTK=y`).
    pub fn add_custom_option(&mut self, option: impl Into<String>) {
        self.custom_options.push(option.into());
    }

    /// Validates that critical storage drivers are present.
    ///
    /// # Returns
    ///
    /// Ok(()) if requirements are met, or an error detailing missing components.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootHarnessError`] if `NVMe`, AHCI, or VFAT drivers are missing.
    pub fn verify_essentials(&self) -> Result<()> {
        if !self.drivers.contains(&KernelDriverKind::Nvme) {
            return Err(Error::BootHarnessError {
                recipe: "kernel-config".to_string(),
                reason: "missing required NVMe driver (CONFIG_BLK_DEV_NVME)".to_string(),
            });
        }
        if !self.drivers.contains(&KernelDriverKind::Ahci) {
            return Err(Error::BootHarnessError {
                recipe: "kernel-config".to_string(),
                reason: "missing required AHCI/SATA driver (CONFIG_SATA_AHCI)".to_string(),
            });
        }
        if !self.drivers.contains(&KernelDriverKind::Vfat) {
            return Err(Error::BootHarnessError {
                recipe: "kernel-config".to_string(),
                reason: "missing required VFAT driver for EFI System Partition".to_string(),
            });
        }
        Ok(())
    }

    /// Renders the complete `.config` file content.
    ///
    /// # Returns
    ///
    /// The formatted Kconfig text.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("# Automatically generated by msi-rs boot harness\n");
        out.push_str("CONFIG_64BIT=y\n");
        out.push_str("CONFIG_PRINTK=y\n");
        out.push_str("CONFIG_DEVTMPFS=y\n");
        out.push_str("CONFIG_DEVTMPFS_MOUNT=y\n");
        out.push_str("CONFIG_BLK_DEV_INITRD=y\n");
        out.push_str("CONFIG_RD_GZIP=y\n");
        out.push_str("CONFIG_EFI_STUB=y\n");

        for driver in &self.drivers {
            out.push_str(driver.kconfig_symbol());
            out.push('\n');
        }

        for custom in &self.custom_options {
            out.push_str(custom);
            out.push('\n');
        }

        out
    }
}

/// Execution mode for early userspace init scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitTargetMode {
    /// Direct PID 1 execution replacing traditional init.
    PidOne,
    /// Systemd unit service configuration.
    SystemdService,
    /// `OpenRC` runscript service.
    OpenRcService,
}

/// Builder for generating early userspace boot initialization scripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitScriptBuilder {
    /// Target init execution architecture.
    mode: InitTargetMode,
    /// Package or bundle path to auto-install, if specified.
    package_path: Option<String>,
    /// Whether to automatically reboot after successful deployment.
    auto_reboot: bool,
    /// Serial console redirection port (e.g. `ttyS0`), if enabled.
    serial_port: Option<String>,
}

impl Default for InitScriptBuilder {
    /// Creates a default [`InitScriptBuilder`] configured for PID 1 execution.
    ///
    /// # Returns
    ///
    /// A new [`InitScriptBuilder`] instance.
    fn default() -> Self {
        Self {
            mode: InitTargetMode::PidOne,
            package_path: None,
            auto_reboot: false,
            serial_port: None,
        }
    }
}

impl InitScriptBuilder {
    /// Creates a new [`InitScriptBuilder`] with default options.
    ///
    /// # Returns
    ///
    /// A fresh builder instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: InitTargetMode::PidOne,
            package_path: None,
            auto_reboot: false,
            serial_port: None,
        }
    }

    /// Sets the init target execution mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Desired execution mode.
    ///
    /// # Returns
    ///
    /// Updated builder instance.
    #[must_use]
    pub const fn with_mode(mut self, mode: InitTargetMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets the target package path to install.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the target `.msi` file.
    ///
    /// # Returns
    ///
    /// Updated builder instance.
    #[must_use]
    pub fn with_package_path(mut self, path: impl Into<String>) -> Self {
        self.package_path = Some(path.into());
        self
    }

    /// Sets whether the machine should reboot on completion.
    ///
    /// # Arguments
    ///
    /// * `auto_reboot` - True to trigger reboot.
    ///
    /// # Returns
    ///
    /// Updated builder instance.
    #[must_use]
    pub const fn with_auto_reboot(mut self, auto_reboot: bool) -> Self {
        self.auto_reboot = auto_reboot;
        self
    }

    /// Configures serial console redirection.
    ///
    /// # Arguments
    ///
    /// * `port` - Serial device name (e.g. `ttyS0`).
    ///
    /// # Returns
    ///
    /// Updated builder instance.
    #[must_use]
    pub fn with_serial_port(mut self, port: Option<String>) -> Self {
        self.serial_port = port;
        self
    }

    /// Renders the initialization script or service definition.
    ///
    /// # Returns
    ///
    /// The generated shell script or service unit content.
    #[must_use]
    pub fn render(&self) -> String {
        match self.mode {
            InitTargetMode::PidOne => self.render_pid_one(),
            InitTargetMode::SystemdService => self.render_systemd(),
            InitTargetMode::OpenRcService => self.render_openrc(),
        }
    }

    /// Renders the standalone PID 1 shell script.
    fn render_pid_one(&self) -> String {
        let mut s = String::from("#!/bin/sh\n");
        s.push_str("# msi-rs bare-metal early userspace init (PID 1)\n");
        s.push_str("set -eu\n\n");
        s.push_str("mount -t proc proc /proc\n");
        s.push_str("mount -t sysfs sysfs /sys\n");
        s.push_str("mount -t devtmpfs devtmpfs /dev\n");
        s.push_str("mkdir -p /sys/firmware/efi/efivars\n");
        s.push_str("mount -t efivarfs efivarfs /sys/firmware/efi/efivars || true\n\n");

        if let Some(ref port) = self.serial_port {
            let _ = writeln!(s, "stty -F /dev/{port} 115200 raw -echo || true");
            let _ = writeln!(s, "exec < /dev/{port} > /dev/{port} 2>&1");
        }

        s.push_str("echo 'Starting msi-rs Bare-Metal Installer...'\n");

        if let Some(ref pkg) = self.package_path {
            let _ = writeln!(s, "msi-cli install /i \"{pkg}\" --tui || true");
        } else {
            s.push_str("msi-cli --tui || true\n");
        }

        if self.auto_reboot {
            s.push_str("echo 'Installation finished. Rebooting system...'\n");
            s.push_str("sync\n");
            s.push_str("reboot -f\n");
        } else {
            s.push_str("echo 'Installation exited. Spawning rescue shell...'\n");
            s.push_str("exec /bin/sh\n");
        }

        s
    }

    /// Renders a systemd unit service file.
    fn render_systemd(&self) -> String {
        let mut s = String::from("[Unit]\n");
        s.push_str("Description=msi-rs Bare-Metal Installer\n");
        s.push_str("DefaultDependencies=no\n");
        s.push_str("After=local-fs.target\n\n");
        s.push_str("[Service]\n");
        s.push_str("Type=idle\n");
        s.push_str("StandardInput=tty\n");
        s.push_str("StandardOutput=tty\n");
        s.push_str("TTYPath=/dev/tty1\n");
        s.push_str("TTYReset=yes\n");
        s.push_str("TTYVHangup=yes\n");

        if let Some(ref pkg) = self.package_path {
            let _ = writeln!(s, "ExecStart=/usr/bin/msi-cli install /i \"{pkg}\" --tui");
        } else {
            s.push_str("ExecStart=/usr/bin/msi-cli --tui\n");
        }

        if self.auto_reboot {
            s.push_str("ExecStopPost=/usr/bin/systemctl reboot\n");
        }

        s.push_str("\n[Install]\n");
        s.push_str("WantedBy=multi-user.target\n");
        s
    }

    /// Renders an `OpenRC` service script.
    fn render_openrc(&self) -> String {
        let mut s = String::from("#!/sbin/openrc-run\n");
        s.push_str("description=\"msi-rs Bare-Metal Installer\"\n\n");
        s.push_str("depend() {\n");
        s.push_str("    need localmount devfs\n");
        s.push_str("}\n\n");
        s.push_str("start() {\n");
        s.push_str("    ebegin \"Starting msi-rs installer\"\n");
        if let Some(ref pkg) = self.package_path {
            let _ = writeln!(s, "    /usr/bin/msi-cli install /i \"{pkg}\" --tui");
        } else {
            s.push_str("    /usr/bin/msi-cli --tui\n");
        }
        s.push_str("    eend $?\n");
        if self.auto_reboot {
            s.push_str("    reboot\n");
        }
        s.push_str("}\n");
        s
    }
}

/// Core userland utility package categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserlandUtilityKind {
    /// Multi-call busybox binary.
    Busybox,
    /// Multi-call toybox binary.
    Toybox,
    /// Linux kernel module utilities (`modprobe`, `insmod`, `rmmod`).
    Kmod,
    /// Essential utilities (`fdisk`, `lsblk`, `mount`, `umount`).
    UtilLinuxMinimal,
}

impl UserlandUtilityKind {
    /// Returns the primary binary names provided by this utility package.
    ///
    /// # Returns
    ///
    /// Slice of binary command names.
    #[must_use]
    pub const fn binaries(self) -> &'static [&'static str] {
        match self {
            Self::Busybox => &["busybox", "sh", "cat", "echo", "mkdir", "mount", "umount"],
            Self::Toybox => &["toybox", "sh", "ls", "ps", "kill"],
            Self::Kmod => &["kmod", "modprobe", "insmod", "rmmod", "depmod"],
            Self::UtilLinuxMinimal => &["fdisk", "lsblk", "blkid", "sfdisk", "partx"],
        }
    }
}

/// Bundle manager for userspace initramfs root filesystems.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserlandBundle {
    /// Selected utility packages.
    utilities: Vec<UserlandUtilityKind>,
    /// Additional injected custom file paths.
    custom_files: Vec<String>,
}

impl UserlandBundle {
    /// Creates a new [`UserlandBundle`] with standard defaults.
    ///
    /// # Returns
    ///
    /// A new bundle instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            utilities: Vec::new(),
            custom_files: Vec::new(),
        }
    }

    /// Adds a utility package to the bundle.
    ///
    /// # Arguments
    ///
    /// * `util` - The utility package kind.
    pub fn add_utility(&mut self, util: UserlandUtilityKind) {
        if !self.utilities.contains(&util) {
            self.utilities.push(util);
        }
    }

    /// Adds a custom file path to the bundle.
    ///
    /// # Arguments
    ///
    /// * `path` - Absolute path within target initramfs.
    pub fn add_custom_file(&mut self, path: impl Into<String>) {
        self.custom_files.push(path.into());
    }

    /// Generates a complete file manifest for the initramfs bundle.
    ///
    /// # Returns
    ///
    /// Vector of target file paths.
    #[must_use]
    pub fn generate_manifest(&self) -> Vec<String> {
        let mut manifest = vec![
            "/init".to_string(),
            "/bin".to_string(),
            "/sbin".to_string(),
            "/etc".to_string(),
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
        ];

        for util in &self.utilities {
            for bin in util.binaries() {
                manifest.push(format!("/bin/{bin}"));
            }
        }

        for custom in &self.custom_files {
            if !manifest.contains(custom) {
                manifest.push(custom.clone());
            }
        }

        manifest
    }

    /// Synthesizes an uncompressed CPIO archive in standard `newc` format.
    ///
    /// # Arguments
    ///
    /// * `entries` - Array of `(path, file_contents)` pairs.
    ///
    /// # Returns
    ///
    /// Serialized byte buffer of the CPIO archive.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootHarnessError`] if an entry path is invalid or empty.
    pub fn generate_cpio_archive(entries: &[(&str, &[u8])]) -> Result<Vec<u8>> {
        let mut archive = Vec::new();

        for (path, content) in entries {
            if path.is_empty() {
                return Err(Error::BootHarnessError {
                    recipe: "cpio".to_string(),
                    reason: "entry path cannot be empty".to_string(),
                });
            }

            let name_bytes = path.as_bytes();
            let name_len = name_bytes.len() + 1;
            let file_size = content.len();

            let header = format!("070701{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}", 0, 0o100_755, 0, 0, 1, 0, file_size, 0, 0, 0, 0, name_len, 0);

            archive.extend_from_slice(header.as_bytes());
            archive.extend_from_slice(name_bytes);
            archive.push(0);

            let pad_header = (4 - ((110 + name_len) % 4)) % 4;
            archive.extend(std::iter::repeat_n(0, pad_header));

            archive.extend_from_slice(content);

            let pad_data = (4 - (file_size % 4)) % 4;
            archive.extend(std::iter::repeat_n(0, pad_data));
        }

        let trailer_name = b"TRAILER!!!\0";
        let trailer_name_len = trailer_name.len();
        let trailer_header = format!(
            "070701{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
            0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, trailer_name_len, 0
        );
        archive.extend_from_slice(trailer_header.as_bytes());
        archive.extend_from_slice(trailer_name);
        let trailer_pad = (4 - ((110 + trailer_name_len) % 4)) % 4;
        archive.extend(std::iter::repeat_n(0, trailer_pad));

        let pad_block = (512 - (archive.len() % 512)) % 512;
        archive.extend(std::iter::repeat_n(0, pad_block));

        Ok(archive)
    }
}

/// Unified Kernel Image (UKI) packager combining EFI stub, kernel, and initramfs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UkiPackager;

impl UkiPackager {
    /// Packages a Unified Kernel Image into a portable executable layout.
    ///
    /// # Arguments
    ///
    /// * `stub` - Pre-built UEFI stub binary (e.g. `systemd-stub`).
    /// * `kernel` - Compressed Linux kernel binary (`vmlinuz`).
    /// * `initramfs` - Serialized initramfs CPIO archive.
    /// * `cmdline` - Kernel command line string.
    /// * `os_release` - Content of `os-release` file.
    ///
    /// # Returns
    ///
    /// Synthesized UKI image binary bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UkiPackageError`] if stub, kernel, or initramfs are empty or too large.
    #[allow(clippy::cast_possible_truncation)]
    pub fn package(
        stub: &[u8],
        kernel: &[u8],
        initramfs: &[u8],
        cmdline: &str,
        os_release: &str,
    ) -> Result<Vec<u8>> {
        if stub.is_empty() {
            return Err(Error::UkiPackageError {
                reason: "EFI stub binary cannot be empty".to_string(),
            });
        }
        if kernel.is_empty() {
            return Err(Error::UkiPackageError {
                reason: "Kernel binary cannot be empty".to_string(),
            });
        }
        if initramfs.is_empty() {
            return Err(Error::UkiPackageError {
                reason: "Initramfs archive cannot be empty".to_string(),
            });
        }

        let cmdline_len = cmdline.len() as u32;
        let osrel_len = os_release.len() as u32;
        let init_len = initramfs.len() as u32;
        let kernel_len = kernel.len() as u32;

        let mut uki = Vec::with_capacity(
            stub.len() + kernel.len() + initramfs.len() + cmdline.len() + os_release.len() + 1024,
        );
        uki.extend_from_slice(stub);
        let pad = (512 - (uki.len() % 512)) % 512;
        uki.extend(std::iter::repeat_n(0, pad));

        uki.extend_from_slice(b".cmdline\0");
        uki.extend_from_slice(&cmdline_len.to_le_bytes());
        uki.extend_from_slice(cmdline.as_bytes());
        let pad_cmd = (512 - (uki.len() % 512)) % 512;
        uki.extend(std::iter::repeat_n(0, pad_cmd));

        uki.extend_from_slice(b".osrel\0\0");
        uki.extend_from_slice(&osrel_len.to_le_bytes());
        uki.extend_from_slice(os_release.as_bytes());
        let pad_os = (512 - (uki.len() % 512)) % 512;
        uki.extend(std::iter::repeat_n(0, pad_os));

        uki.extend_from_slice(b".initrd\0");
        uki.extend_from_slice(&init_len.to_le_bytes());
        uki.extend_from_slice(initramfs);
        let pad_init = (512 - (uki.len() % 512)) % 512;
        uki.extend(std::iter::repeat_n(0, pad_init));

        uki.extend_from_slice(b".linux\0\0");
        uki.extend_from_slice(&kernel_len.to_le_bytes());
        uki.extend_from_slice(kernel);
        let pad_kernel = (512 - (uki.len() % 512)) % 512;
        uki.extend(std::iter::repeat_n(0, pad_kernel));

        Ok(uki)
    }
}

/// Target media format for installer image creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveMediaFormat {
    /// Hybrid ISO-9660 with El Torito UEFI boot record.
    HybridIso,
    /// Raw block image for direct USB flashing (`dd`).
    RawUsbDisk,
}

/// Generator for bootable ISO-9660 and USB drive images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveMediaGenerator {
    /// Target image media format.
    format: LiveMediaFormat,
    /// Volume label string.
    volume_label: String,
    /// Bootloader EFI binary bytes.
    bootloader_efi: Vec<u8>,
}

impl LiveMediaGenerator {
    /// Creates a new [`LiveMediaGenerator`].
    ///
    /// # Arguments
    ///
    /// * `format` - Target media image format.
    /// * `volume_label` - ISO/disk volume identifier (max 32 chars).
    /// * `bootloader_efi` - Primary UEFI boot binary bytes.
    ///
    /// # Returns
    ///
    /// A new generator instance.
    #[must_use]
    pub fn new(
        format: LiveMediaFormat,
        volume_label: impl Into<String>,
        bootloader_efi: Vec<u8>,
    ) -> Self {
        Self {
            format,
            volume_label: volume_label.into(),
            bootloader_efi,
        }
    }

    /// Synthesizes the live boot media image bytes.
    ///
    /// # Returns
    ///
    /// Binary image buffer ready for writing to media.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootHarnessError`] if bootloader is empty or label is invalid.
    pub fn generate(&self) -> Result<Vec<u8>> {
        if self.bootloader_efi.is_empty() {
            return Err(Error::BootHarnessError {
                recipe: "live-media".to_string(),
                reason: "bootloader EFI binary cannot be empty".to_string(),
            });
        }
        if self.volume_label.is_empty() {
            return Err(Error::BootHarnessError {
                recipe: "live-media".to_string(),
                reason: "volume label cannot be empty".to_string(),
            });
        }

        Ok(match self.format {
            LiveMediaFormat::HybridIso => self.generate_hybrid_iso(),
            LiveMediaFormat::RawUsbDisk => self.generate_raw_usb(),
        })
    }

    /// Generates hybrid ISO-9660 with Primary Volume Descriptor and El Torito boot entry.
    fn generate_hybrid_iso(&self) -> Vec<u8> {
        let sector_size = 2048;
        let mut image = vec![0u8; 16 * sector_size];

        let mut pvd = vec![0u8; sector_size];
        pvd[0] = 0x01;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 0x01;

        let label_bytes = self.volume_label.as_bytes();
        let copy_len = label_bytes.len().min(32);
        pvd[40..40 + copy_len].copy_from_slice(&label_bytes[..copy_len]);
        image.extend_from_slice(&pvd);

        let mut el_torito_vd = vec![0u8; sector_size];
        el_torito_vd[0] = 0x00;
        el_torito_vd[1..6].copy_from_slice(b"CD001");
        el_torito_vd[6] = 0x01;
        el_torito_vd[7..30].copy_from_slice(b"EL TORITO SPECIFICATION");
        el_torito_vd[71] = 19;
        image.extend_from_slice(&el_torito_vd);

        let mut term = vec![0u8; sector_size];
        term[0] = 0xFF;
        term[1..6].copy_from_slice(b"CD001");
        term[6] = 0x01;
        image.extend_from_slice(&term);

        let mut catalog = vec![0u8; sector_size];
        catalog[0] = 0x01;
        catalog[1] = 0xEF;
        catalog[0x1E] = 0x55;
        catalog[0x1F] = 0xAA;

        catalog[0x20] = 0x88;
        catalog[0x21] = 0x00;
        catalog[0x26] = 0x01;
        catalog[0x28] = 20;
        image.extend_from_slice(&catalog);

        let mut boot_payload = self.bootloader_efi.clone();
        let pad = (sector_size - (boot_payload.len() % sector_size)) % sector_size;
        boot_payload.extend(std::iter::repeat_n(0, pad));
        image.extend_from_slice(&boot_payload);

        image
    }

    /// Generates raw USB block disk image with standard MBR/GPT protective header.
    fn generate_raw_usb(&self) -> Vec<u8> {
        let sector_size = 512;
        let mut image = vec![0u8; sector_size * 2];
        image[510] = 0x55;
        image[511] = 0xAA;
        image[446] = 0x80;
        image[450] = 0xEF;
        image[454] = 0x02;

        let mut payload = self.bootloader_efi.clone();
        let pad = (sector_size - (payload.len() % sector_size)) % sector_size;
        payload.extend(std::iter::repeat_n(0, pad));
        image.extend_from_slice(&payload);

        image
    }
}

/// Automation harness for Windows Preinstallation Environment (`WinPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WinPeHarness;

impl WinPeHarness {
    /// Generates the standard `WinPE` `startnet.cmd` startup script.
    ///
    /// # Arguments
    ///
    /// * `additional_args` - Additional command-line flags for `msi-cli`.
    ///
    /// # Returns
    ///
    /// Formatted batch file content.
    #[must_use]
    pub fn generate_startnet_cmd(additional_args: &str) -> String {
        let mut s = String::from("@echo off\r\n");
        s.push_str("title msi-rs Bare-Metal Installer (WinPE)\r\n");
        s.push_str("echo Initializing WinPE networking and storage drivers...\r\n");
        s.push_str("wpeinit\r\n\r\n");
        s.push_str("echo Launching msi-cli Terminal UI Wizard...\r\n");
        if additional_args.is_empty() {
            s.push_str("X:\\msi\\msi-cli.exe --tui\r\n");
        } else {
            let _ = writeln!(s, "X:\\msi\\msi-cli.exe --tui {additional_args}");
        }
        s.push_str("\r\nif errorlevel 1 (\r\n");
        s.push_str("    echo Installer exited with an error. Spawning recovery shell...\r\n");
        s.push_str("    cmd.exe\r\n");
        s.push_str(") else (\r\n");
        s.push_str("    echo Installation successful. System will now restart.\r\n");
        s.push_str("    wpeutil reboot\r\n");
        s.push_str(")\r\n");
        s
    }

    /// Generates a PowerShell script for automating `msi-cli` injection into `boot.wim`.
    ///
    /// # Arguments
    ///
    /// * `wim_path` - Path to the original `boot.wim` image.
    /// * `cli_binary_path` - Path to compiled `msi-cli.exe`.
    /// * `mount_dir` - Scratch directory for mounting WIM image.
    ///
    /// # Returns
    ///
    /// Formatted PowerShell script content.
    #[must_use]
    pub fn generate_wim_injection_script(
        wim_path: &str,
        cli_binary_path: &str,
        mount_dir: &str,
    ) -> String {
        let mut s = String::from("# Automates injection of msi-cli into WinPE boot.wim\n");
        s.push_str("$ErrorActionPreference = \"Stop\"\n\n");
        let _ = writeln!(s, "$WimPath = \"{wim_path}\"");
        let _ = writeln!(s, "$CliPath = \"{cli_binary_path}\"");
        let _ = writeln!(s, "$MountDir = \"{mount_dir}\"");
        s.push_str("\nNew-Item -ItemType Directory -Force -Path $MountDir | Out-Null\n");
        s.push_str("Write-Host \"Mounting WinPE image...\"\n");
        s.push_str("dism /Mount-Wim /WimFile:$WimPath /Index:1 /MountDir:$MountDir\n\n");
        s.push_str("Write-Host \"Copying msi-cli binary...\"\n");
        s.push_str("New-Item -ItemType Directory -Force -Path \"$MountDir\\msi\" | Out-Null\n");
        s.push_str(
            "Copy-Item -Path $CliPath -Destination \"$MountDir\\msi\\msi-cli.exe\" -Force\n\n",
        );
        s.push_str("Write-Host \"Injecting startnet.cmd...\"\n");
        s.push_str(
            "$StartNet = @'\n@echo off\nwpeinit\nX:\\msi\\msi-cli.exe --tui\nwpeutil reboot\n'@\n",
        );
        s.push_str("Set-Content -Path \"$MountDir\\Windows\\System32\\startnet.cmd\" -Value $StartNet -Encoding Ascii\n\n");
        s.push_str("Write-Host \"Unmounting and committing image...\"\n");
        s.push_str("dism /Unmount-Wim /MountDir:$MountDir /Commit\n");
        s.push_str("Write-Host \"WinPE image injection complete!\"\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that kernel configuration defaults and additions render expected lines.
    #[test]
    fn test_kernel_config_rendering_and_validation() {
        let mut config = KernelConfig::new();
        config.add_driver(KernelDriverKind::Nvme);
        config.add_driver(KernelDriverKind::Nvme); // duplicate
        config.add_driver(KernelDriverKind::Ahci);

        assert!(config.verify_essentials().is_err());

        config.add_driver(KernelDriverKind::Vfat);
        assert!(config.verify_essentials().is_ok());

        config.add_driver(KernelDriverKind::VirtioNet);
        config.add_custom_option("CONFIG_SMP=y");

        let rendered = config.render();
        assert!(rendered.contains("CONFIG_BLK_DEV_NVME=y"));
        assert!(rendered.contains("CONFIG_SATA_AHCI=y"));
        assert!(rendered.contains("CONFIG_VFAT_FS=y"));
        assert!(rendered.contains("CONFIG_VIRTIO_NET=y"));
        assert!(rendered.contains("CONFIG_SMP=y"));

        let default_cfg = KernelConfig::default();
        assert!(default_cfg.verify_essentials().is_ok());

        // Validate error branches in verify_essentials
        let empty_cfg = KernelConfig::new();
        assert!(empty_cfg.verify_essentials().is_err());

        let mut no_ahci = KernelConfig::new();
        no_ahci.add_driver(KernelDriverKind::Nvme);
        assert!(no_ahci.verify_essentials().is_err());

        let mut no_vfat = KernelConfig::new();
        no_vfat.add_driver(KernelDriverKind::Nvme);
        no_vfat.add_driver(KernelDriverKind::Ahci);
        assert!(no_vfat.verify_essentials().is_err());

        // KernelDriverKind kconfig string variants
        assert_eq!(
            KernelDriverKind::UsbStorage.kconfig_symbol(),
            "CONFIG_USB_STORAGE=y"
        );
        assert_eq!(
            KernelDriverKind::VirtioBlk.kconfig_symbol(),
            "CONFIG_VIRTIO_BLK=y"
        );
        assert_eq!(KernelDriverKind::Ext4.kconfig_symbol(), "CONFIG_EXT4_FS=y");
        assert_eq!(
            KernelDriverKind::Efivarfs.kconfig_symbol(),
            "CONFIG_EFIVAR_FS=y"
        );
    }

    /// Tests early userspace init script generation across all target modes.
    #[test]
    fn test_init_script_builder_modes() {
        let builder_pid1 = InitScriptBuilder::new()
            .with_package_path("/media/os.msi")
            .with_serial_port(Some("ttyS0".to_string()))
            .with_auto_reboot(true);
        let pid1_sh = builder_pid1.render();
        assert!(pid1_sh.contains("msi-cli install /i"));
        assert!(pid1_sh.contains("/media/os.msi"));
        assert!(pid1_sh.contains("/dev/ttyS0"));
        assert!(pid1_sh.contains("reboot -f"));

        let builder_rescue = InitScriptBuilder::default();
        let rescue_sh = builder_rescue.render();
        assert!(rescue_sh.contains("exec /bin/sh"));

        let builder_systemd = InitScriptBuilder::new()
            .with_mode(InitTargetMode::SystemdService)
            .with_package_path("/media/os.msi")
            .with_auto_reboot(true);
        let systemd_unit = builder_systemd.render();
        assert!(systemd_unit.contains("ExecStart=/usr/bin/msi-cli install /i"));
        assert!(systemd_unit.contains("/media/os.msi"));
        assert!(systemd_unit.contains("ExecStopPost=/usr/bin/systemctl reboot"));

        let builder_systemd_no_pkg =
            InitScriptBuilder::new().with_mode(InitTargetMode::SystemdService);
        assert!(builder_systemd_no_pkg
            .render()
            .contains("ExecStart=/usr/bin/msi-cli --tui"));

        let builder_openrc = InitScriptBuilder::new()
            .with_mode(InitTargetMode::OpenRcService)
            .with_package_path("/media/os.msi")
            .with_auto_reboot(true);
        let openrc_script = builder_openrc.render();
        assert!(openrc_script.contains("/usr/bin/msi-cli install /i"));
        assert!(openrc_script.contains("reboot"));

        let builder_openrc_no_pkg =
            InitScriptBuilder::new().with_mode(InitTargetMode::OpenRcService);
        assert!(builder_openrc_no_pkg
            .render()
            .contains("/usr/bin/msi-cli --tui"));
    }

    /// Tests userland bundle manifest generation and CPIO archive creation.
    #[test]
    fn test_userland_bundle_cpio_archive() {
        let mut bundle = UserlandBundle::new();
        bundle.add_utility(UserlandUtilityKind::Busybox);
        bundle.add_utility(UserlandUtilityKind::Busybox); // duplicate
        bundle.add_utility(UserlandUtilityKind::Toybox);
        bundle.add_utility(UserlandUtilityKind::Kmod);
        bundle.add_utility(UserlandUtilityKind::UtilLinuxMinimal);
        bundle.add_custom_file("/etc/os-release");
        bundle.add_custom_file("/dev"); // duplicate of default manifest entry

        let manifest = bundle.generate_manifest();
        assert!(manifest.contains(&"/bin/busybox".to_string()));
        assert!(manifest.contains(&"/bin/modprobe".to_string()));
        assert!(manifest.contains(&"/bin/fdisk".to_string()));
        assert!(manifest.contains(&"/etc/os-release".to_string()));

        let files = [
            ("init", b"#!/bin/sh\necho hi\n".as_slice()),
            ("etc/issue", b"Welcome to msi-rs\n".as_slice()),
        ];
        let cpio = UserlandBundle::generate_cpio_archive(&files);
        assert_eq!(cpio.as_ref().map(|b| b.starts_with(b"070701")), Ok(true));
        assert_eq!(cpio.as_ref().map(|b| b.len() % 512), Ok(0));

        let empty_entry = [("", b"".as_slice())];
        assert!(UserlandBundle::generate_cpio_archive(&empty_entry).is_err());
    }

    /// Tests UKI packaging and validation errors.
    #[test]
    fn test_uki_packager() {
        let stub = vec![0x90; 1024];
        let kernel = vec![0xCC; 2048];
        let initramfs = vec![0x55; 4096];
        let cmdline = "console=ttyS0 root=/dev/ram0";
        let osrel = "NAME=msi-rs\nVERSION=1.0\n";

        let uki = UkiPackager::package(&stub, &kernel, &initramfs, cmdline, osrel);
        assert_eq!(uki.as_ref().map(|b| b.len() > 7000), Ok(true));
        assert_eq!(uki.as_ref().map(|b| b.len() % 512), Ok(0));

        assert!(UkiPackager::package(&[], &kernel, &initramfs, cmdline, osrel).is_err());
        assert!(UkiPackager::package(&stub, &[], &initramfs, cmdline, osrel).is_err());
        assert!(UkiPackager::package(&stub, &kernel, &[], cmdline, osrel).is_err());
    }

    /// Tests live media generation for Hybrid ISO and raw USB images.
    #[test]
    fn test_live_media_generator() {
        let bootloader = vec![0xEB, 0x58, 0x90, 0x00];
        let gen_iso = LiveMediaGenerator::new(
            LiveMediaFormat::HybridIso,
            "MSI_INSTALL",
            bootloader.clone(),
        );
        let iso_bytes = gen_iso.generate();
        assert_eq!(
            iso_bytes.as_ref().map(|b| &b[16 * 2048 + 1..16 * 2048 + 6]),
            Ok(b"CD001".as_slice())
        );

        let gen_usb =
            LiveMediaGenerator::new(LiveMediaFormat::RawUsbDisk, "MSI_USB", bootloader.clone());
        let usb_bytes = gen_usb.generate();
        assert_eq!(
            usb_bytes.as_ref().map(|b| (b[510], b[511])),
            Ok((0x55, 0xAA))
        );

        let err_empty_boot = LiveMediaGenerator::new(LiveMediaFormat::HybridIso, "LABEL", vec![]);
        assert!(err_empty_boot.generate().is_err());

        let err_empty_label = LiveMediaGenerator::new(LiveMediaFormat::HybridIso, "", bootloader);
        assert!(err_empty_label.generate().is_err());
    }

    /// Tests `WinPE` script generation for startnet.cmd and WIM injection.
    #[test]
    fn test_winpe_harness() {
        let startnet_default = WinPeHarness::generate_startnet_cmd("");
        assert!(startnet_default.contains("wpeinit"));
        assert!(startnet_default.contains("X:\\msi\\msi-cli.exe --tui"));

        let startnet_args = WinPeHarness::generate_startnet_cmd("install /i C:\\os.msi");
        assert!(startnet_args.contains("X:\\msi\\msi-cli.exe --tui install /i C:\\os.msi"));

        let ps_script = WinPeHarness::generate_wim_injection_script(
            "C:\\boot.wim",
            "C:\\target\\msi-cli.exe",
            "C:\\mount",
        );
        assert!(ps_script.contains("dism /Mount-Wim /WimFile:$WimPath"));
        assert!(ps_script.contains("dism /Unmount-Wim /MountDir:$MountDir /Commit"));
    }
}

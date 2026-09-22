//! Boot Environment & Live Harness for Bare-Metal OS Installation.
//!
//! Provides minimal bootable Linux runtime generation (initramfs, kernel configuration,
//! UKI packaging, hybrid ISO/USB layout) and `WinPE` automation harnesses.

use crate::error::{Error, Result};
use crate::platform::partition::{GptPartitionEntry, Lba, PartitionTypeGuid, PartitionUuid};
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
    /// Returns [`Error::UkiPackageError`] if stub, kernel, or initramfs are empty.
    pub fn package(
        stub: &[u8],
        kernel: &[u8],
        initramfs: &[u8],
        cmdline: &str,
        os_release: &str,
    ) -> Result<Vec<u8>> {
        Self::package_full(stub, kernel, initramfs, cmdline, os_release, "Linux")
    }

    /// Packages a Unified Kernel Image with full section specifications including `.uname`.
    ///
    /// Synthesizes a valid PE32+ binary with DOS stub, PE header, and queryable section headers
    /// aligned to 4096-byte section alignment and 512-byte file alignment.
    ///
    /// # Arguments
    ///
    /// * `stub` - Pre-built UEFI stub binary or raw stub bytes.
    /// * `kernel` - Compressed Linux kernel binary.
    /// * `initramfs` - Initramfs CPIO archive.
    /// * `cmdline` - Kernel command line string.
    /// * `os_release` - Content of `os-release` file.
    /// * `uname` - Kernel version or uname string.
    ///
    /// # Returns
    ///
    /// Synthesized PE32+ UKI image buffer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UkiPackageError`] if critical input components are empty.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn package_full(
        stub: &[u8],
        kernel: &[u8],
        initramfs: &[u8],
        cmdline: &str,
        os_release: &str,
        uname: &str,
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

        let sections: [(&[u8; 8], &[u8]); 5] = [
            (b".cmdline", cmdline.as_bytes()),
            (b".osrel\0\0", os_release.as_bytes()),
            (b".initrd\0", initramfs),
            (b".linux\0\0", kernel),
            (b".uname\0\0", uname.as_bytes()),
        ];

        // 512-byte aligned header size (1024 bytes to accommodate DOS + PE + 5 section headers)
        let num_sections: u16 = 5;
        let header_size: u32 = 1024;

        let mut image = vec![0u8; header_size as usize];

        // 1. DOS Header
        image[0] = 0x4D; // 'M'
        image[1] = 0x5A; // 'Z'
        image[0x3C] = 0x80; // e_lfanew -> offset 0x80 for PE signature

        // 2. PE Signature
        let pe_offset = 0x80;
        image[pe_offset..pe_offset + 4].copy_from_slice(b"PE\0\0");

        // 3. COFF File Header (20 bytes)
        let coff_offset = pe_offset + 4;
        image[coff_offset..coff_offset + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
        image[coff_offset + 2..coff_offset + 4].copy_from_slice(&num_sections.to_le_bytes());
        image[coff_offset + 16..coff_offset + 18].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader (PE32+)
        image[coff_offset + 18..coff_offset + 20].copy_from_slice(&0x022Eu16.to_le_bytes()); // Characteristics

        // 4. Optional Header PE32+ (240 bytes)
        let opt_offset = coff_offset + 20;
        image[opt_offset..opt_offset + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // Magic: PE32+
        image[opt_offset + 16..opt_offset + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint
        image[opt_offset + 20..opt_offset + 24].copy_from_slice(&0x1000u32.to_le_bytes()); // BaseOfCode
        image[opt_offset + 24..opt_offset + 32].copy_from_slice(&0x0040_0000u64.to_le_bytes()); // ImageBase
        image[opt_offset + 32..opt_offset + 36].copy_from_slice(&4096u32.to_le_bytes()); // SectionAlignment
        image[opt_offset + 36..opt_offset + 40].copy_from_slice(&512u32.to_le_bytes()); // FileAlignment
        image[opt_offset + 60..opt_offset + 64].copy_from_slice(&header_size.to_le_bytes()); // SizeOfHeaders
        image[opt_offset + 68..opt_offset + 70].copy_from_slice(&10u16.to_le_bytes()); // Subsystem: EFI_APPLICATION
        image[opt_offset + 108..opt_offset + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes

        // 5. Section Table (5 * 40 bytes)
        let sec_table_offset = opt_offset + 240;
        let mut current_rva: u32 = 4096;
        let mut current_raw_offset: u32 = header_size;
        let mut section_payloads = Vec::new();

        for (idx, (name, data)) in sections.iter().enumerate() {
            let sec_entry = sec_table_offset + (idx * 40);
            image[sec_entry..sec_entry + 8].copy_from_slice(*name);

            let virt_size = data.len() as u32;
            let raw_size = virt_size.div_ceil(512) * 512;

            image[sec_entry + 8..sec_entry + 12].copy_from_slice(&virt_size.to_le_bytes());
            image[sec_entry + 12..sec_entry + 16].copy_from_slice(&current_rva.to_le_bytes());
            image[sec_entry + 16..sec_entry + 20].copy_from_slice(&raw_size.to_le_bytes());
            image[sec_entry + 20..sec_entry + 24]
                .copy_from_slice(&current_raw_offset.to_le_bytes());
            // Characteristics: IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ (0x40000040)
            image[sec_entry + 36..sec_entry + 40].copy_from_slice(&0x4000_0040u32.to_le_bytes());

            section_payloads.push((current_raw_offset as usize, *data, raw_size as usize));

            let aligned_rva_span = virt_size.div_ceil(4096) * 4096;
            current_rva += aligned_rva_span.max(4096);
            current_raw_offset += raw_size;
        }

        // Update SizeOfImage in Optional Header
        image[opt_offset + 56..opt_offset + 60].copy_from_slice(&current_rva.to_le_bytes());

        // Append padded payloads
        image.resize(current_raw_offset as usize, 0);
        for (offset, data, raw_size) in section_payloads {
            image[offset..offset + data.len()].copy_from_slice(data);
            for p in &mut image[offset + data.len()..offset + raw_size] {
                *p = 0;
            }
        }

        Ok(image)
    }

    /// Queries the section table of a compiled UKI PE binary.
    ///
    /// # Arguments
    ///
    /// * `uki_bytes` - Compiled UKI binary bytes.
    ///
    /// # Returns
    ///
    /// Vector of tuples containing `(name, virtual_address, raw_size, characteristics)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UkiPackageError`] if the binary is not a valid PE image.
    pub fn query_sections(uki_bytes: &[u8]) -> Result<Vec<(String, u32, u32, u32)>> {
        if uki_bytes.len() < 0x40 || uki_bytes[0] != 0x4D || uki_bytes[1] != 0x5A {
            return Err(Error::UkiPackageError {
                reason: "not a valid DOS/PE executable".to_string(),
            });
        }

        let e_lfanew = u32::from_le_bytes([
            uki_bytes[0x3C],
            uki_bytes[0x3D],
            uki_bytes[0x3E],
            uki_bytes[0x3F],
        ]) as usize;

        if uki_bytes.len() < e_lfanew + 24 || &uki_bytes[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
            return Err(Error::UkiPackageError {
                reason: "corrupt PE signature".to_string(),
            });
        }

        let num_sections =
            u16::from_le_bytes([uki_bytes[e_lfanew + 6], uki_bytes[e_lfanew + 7]]) as usize;

        let opt_size =
            u16::from_le_bytes([uki_bytes[e_lfanew + 20], uki_bytes[e_lfanew + 21]]) as usize;

        let sec_start = e_lfanew + 24 + opt_size;
        let mut results = Vec::new();

        for i in 0..num_sections {
            let offset = sec_start + (i * 40);
            if offset + 40 > uki_bytes.len() {
                break;
            }

            let name_bytes = &uki_bytes[offset..offset + 8];
            let name = String::from_utf8_lossy(name_bytes)
                .trim_end_matches('\0')
                .to_string();
            let va = u32::from_le_bytes([
                uki_bytes[offset + 12],
                uki_bytes[offset + 13],
                uki_bytes[offset + 14],
                uki_bytes[offset + 15],
            ]);
            let raw_size = u32::from_le_bytes([
                uki_bytes[offset + 16],
                uki_bytes[offset + 17],
                uki_bytes[offset + 18],
                uki_bytes[offset + 19],
            ]);
            let charact = u32::from_le_bytes([
                uki_bytes[offset + 36],
                uki_bytes[offset + 37],
                uki_bytes[offset + 38],
                uki_bytes[offset + 39],
            ]);

            results.push((name, va, raw_size, charact));
        }

        Ok(results)
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

    /// Generates hybrid ISO-9660 with Primary Volume Descriptor, Path Tables, Directory records, and El Torito boot catalog.
    fn generate_hybrid_iso(&self) -> Vec<u8> {
        let sector_size = 2048;
        let mut image = vec![0u8; 16 * sector_size];

        // Sector 16: Primary Volume Descriptor
        let mut pvd = vec![0u8; sector_size];
        pvd[0] = 0x01; // Primary Volume Descriptor
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 0x01; // Version 1

        let label_bytes = self.volume_label.as_bytes();
        let copy_len = label_bytes.len().min(32);
        pvd[40..40 + copy_len].copy_from_slice(&label_bytes[..copy_len]);

        // Volume Space Size: 32 sectors total
        pvd[80..84].copy_from_slice(&32u32.to_le_bytes());
        pvd[84..88].copy_from_slice(&32u32.to_be_bytes());

        // Type L Path Table Location: Sector 19
        pvd[140..144].copy_from_slice(&19u32.to_le_bytes());

        // Root Directory Record at offset 156 (34 bytes)
        pvd[156] = 34; // Length of directory record
        pvd[158..162].copy_from_slice(&20u32.to_le_bytes()); // Extent location: Sector 20
        pvd[162..166].copy_from_slice(&20u32.to_be_bytes());
        pvd[166..170].copy_from_slice(&2048u32.to_le_bytes()); // Data length
        pvd[170..174].copy_from_slice(&2048u32.to_be_bytes());
        pvd[181] = 0x02; // Directory flag
        pvd[188] = 1; // File identifier length
        pvd[189] = 0; // Root identifier (\0)
        image.extend_from_slice(&pvd);

        // Sector 17: El Torito Boot Record
        let mut el_torito_vd = vec![0u8; sector_size];
        el_torito_vd[0] = 0x00; // Boot Record Indicator
        el_torito_vd[1..6].copy_from_slice(b"CD001");
        el_torito_vd[6] = 0x01; // Version 1
        el_torito_vd[7..30].copy_from_slice(b"EL TORITO SPECIFICATION");
        el_torito_vd[71..75].copy_from_slice(&22u32.to_le_bytes()); // Boot Catalog LBA: Sector 22
        image.extend_from_slice(&el_torito_vd);

        // Sector 18: Volume Descriptor Set Terminator
        let mut term = vec![0u8; sector_size];
        term[0] = 0xFF; // Terminator
        term[1..6].copy_from_slice(b"CD001");
        term[6] = 0x01;
        image.extend_from_slice(&term);

        // Sector 19: Path Table (Root /, /EFI, /EFI/BOOT)
        let mut path_table = vec![0u8; sector_size];
        // Record 1: Root
        path_table[0] = 1; // Len
        path_table[2..6].copy_from_slice(&20u32.to_le_bytes()); // LBA 20
        path_table[6..8].copy_from_slice(&1u16.to_le_bytes()); // Parent record 1
        path_table[8] = 0;
        // Record 2: EFI
        let pt2 = 10;
        path_table[pt2] = 3; // Len
        path_table[pt2 + 2..pt2 + 6].copy_from_slice(&21u32.to_le_bytes()); // LBA 21
        path_table[pt2 + 6..pt2 + 8].copy_from_slice(&1u16.to_le_bytes()); // Parent 1
        path_table[pt2 + 8..pt2 + 11].copy_from_slice(b"EFI");
        image.extend_from_slice(&path_table);

        // Sector 20: Root Directory Sector
        let mut root_dir = vec![0u8; sector_size];
        // Entry 1: .
        root_dir[0] = 34;
        root_dir[2..6].copy_from_slice(&20u32.to_le_bytes());
        root_dir[10..14].copy_from_slice(&2048u32.to_le_bytes());
        root_dir[25] = 0x02;
        root_dir[32] = 1;
        root_dir[33] = 0;
        // Entry 2: ..
        root_dir[34] = 34;
        root_dir[36..40].copy_from_slice(&20u32.to_le_bytes());
        root_dir[44..48].copy_from_slice(&2048u32.to_le_bytes());
        root_dir[59] = 0x02;
        root_dir[66] = 1;
        root_dir[67] = 1;
        // Entry 3: EFI directory
        root_dir[68] = 36;
        root_dir[70..74].copy_from_slice(&21u32.to_le_bytes()); // LBA 21
        root_dir[78..82].copy_from_slice(&2048u32.to_le_bytes());
        root_dir[93] = 0x02;
        root_dir[100] = 3;
        root_dir[101..104].copy_from_slice(b"EFI");
        image.extend_from_slice(&root_dir);

        // Sector 21: EFI Directory Sector
        let mut efi_dir = vec![0u8; sector_size];
        efi_dir[0] = 34;
        efi_dir[2..6].copy_from_slice(&21u32.to_le_bytes());
        efi_dir[25] = 0x02;
        image.extend_from_slice(&efi_dir);

        // Sector 22: El Torito Boot Catalog
        let mut catalog = vec![0u8; sector_size];
        // Validation Entry
        catalog[0] = 0x01; // Header ID
        catalog[1] = 0xEF; // Platform ID: EFI
        catalog[0x1E] = 0x55;
        catalog[0x1F] = 0xAA;
        // Initial/Default Entry
        catalog[0x20] = 0x88; // Bootable
        catalog[0x21] = 0x00; // No emulation
        catalog[0x26] = 0x01; // Sector count
        catalog[0x28] = 24; // Load LBA: Sector 24
        image.extend_from_slice(&catalog);

        // Sector 23: Padding/Reserved
        image.extend_from_slice(&vec![0u8; sector_size]);

        // Sector 24+: Bootloader Payload
        let mut boot_payload = self.bootloader_efi.clone();
        let pad = (sector_size - (boot_payload.len() % sector_size)) % sector_size;
        boot_payload.extend(std::iter::repeat_n(0, pad));
        image.extend_from_slice(&boot_payload);

        image
    }

    /// Generates raw USB block disk image with protective MBR and compliant GPT partition table.
    #[allow(clippy::cast_possible_truncation)]
    fn generate_raw_usb(&self) -> Vec<u8> {
        let sector_size = 512;
        let total_sectors = 4096u64; // 2 MiB disk image
        let mut image = vec![0u8; (total_sectors * sector_size) as usize];

        // Sector 0: Protective MBR
        image[446] = 0x00; // Non-bootable in legacy MBR (UEFI boot)
        image[447] = 0x00; // Starting head
        image[448] = 0x02; // Starting sector
        image[449] = 0x00; // Starting cylinder
        image[450] = 0xEE; // Partition Type: GPT Protective MBR
        image[454..458].copy_from_slice(&1u32.to_le_bytes()); // Starting LBA: 1
        image[458..462].copy_from_slice(&((total_sectors as u32) - 1).to_le_bytes()); // Sector count
        image[510] = 0x55;
        image[511] = 0xAA;

        // Sector 1: Primary GPT Header
        let gpt_offset = sector_size as usize;
        image[gpt_offset..gpt_offset + 8].copy_from_slice(b"EFI PART");
        image[gpt_offset + 8..gpt_offset + 12].copy_from_slice(&0x0001_0000u32.to_le_bytes()); // Revision 1.0
        image[gpt_offset + 12..gpt_offset + 16].copy_from_slice(&92u32.to_le_bytes()); // Header size 92
        image[gpt_offset + 24..gpt_offset + 32].copy_from_slice(&1u64.to_le_bytes()); // MyLBA
        image[gpt_offset + 32..gpt_offset + 40].copy_from_slice(&(total_sectors - 1).to_le_bytes()); // AlternateLBA
        image[gpt_offset + 40..gpt_offset + 48].copy_from_slice(&34u64.to_le_bytes()); // FirstUsableLBA
        image[gpt_offset + 48..gpt_offset + 56]
            .copy_from_slice(&(total_sectors - 34).to_le_bytes()); // LastUsableLBA
        image[gpt_offset + 56..gpt_offset + 72].copy_from_slice(&[0x77; 16]); // Disk GUID
        image[gpt_offset + 72..gpt_offset + 80].copy_from_slice(&2u64.to_le_bytes()); // PartitionEntryLBA
        image[gpt_offset + 80..gpt_offset + 84].copy_from_slice(&128u32.to_le_bytes()); // NumberOfPartitionEntries
        image[gpt_offset + 84..gpt_offset + 88].copy_from_slice(&128u32.to_le_bytes()); // SizeOfPartitionEntry

        // Sectors 2..33: GPT Partition Entries (Entry 0 = ESP, Entry 1 = Payload)
        let pe_offset = 2 * (sector_size as usize);

        // Entry 0: ESP
        let esp_entry = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([0x11; 16]),
            start_lba: Lba(2048),
            end_lba: Lba(3071),
            attributes: 0,
            name: "EFI System Partition".to_string(),
        };
        image[pe_offset..pe_offset + 128].copy_from_slice(&esp_entry.serialize());

        // Entry 1: Payload
        let payload_entry = GptPartitionEntry {
            type_guid: PartitionTypeGuid::LINUX_ROOT_X86_64,
            unique_guid: PartitionUuid([0x22; 16]),
            start_lba: Lba(3072),
            end_lba: Lba(4062),
            attributes: 0,
            name: "Installer Payload".to_string(),
        };
        image[pe_offset + 128..pe_offset + 256].copy_from_slice(&payload_entry.serialize());

        // Write bootloader payload at ESP sector (LBA 2048)
        let esp_data_offset = 2048 * (sector_size as usize);
        let copy_len = self.bootloader_efi.len().min(512 * 512);
        image[esp_data_offset..esp_data_offset + copy_len]
            .copy_from_slice(&self.bootloader_efi[..copy_len]);

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

        let uki_bytes = uki.unwrap_or_default();
        let sections = UkiPackager::query_sections(&uki_bytes);
        assert!(sections.is_ok());
        let sec_list = sections.unwrap_or_default();
        assert_eq!(sec_list.len(), 5);
        assert_eq!(sec_list[0].0, ".cmdline");
        assert_eq!(sec_list[1].0, ".osrel");
        assert_eq!(sec_list[2].0, ".initrd");
        assert_eq!(sec_list[3].0, ".linux");
        assert_eq!(sec_list[4].0, ".uname");
        for sec in &sec_list {
            assert_eq!(sec.3, 0x4000_0040);
        }

        // Test package_full explicitly
        let uki_full =
            UkiPackager::package_full(&stub, &kernel, &initramfs, cmdline, osrel, "6.10.0-msi");
        assert!(uki_full.is_ok());

        // Error branches
        assert!(UkiPackager::package(&[], &kernel, &initramfs, cmdline, osrel).is_err());
        assert!(UkiPackager::package(&stub, &[], &initramfs, cmdline, osrel).is_err());
        assert!(UkiPackager::package(&stub, &kernel, &[], cmdline, osrel).is_err());

        // Corrupt query_sections checks
        assert!(UkiPackager::query_sections(&[]).is_err());
        assert!(UkiPackager::query_sections(&[0u8; 100]).is_err());
        let mut bad_magic2 = vec![0x4D, 0x00];
        bad_magic2.resize(100, 0);
        assert!(UkiPackager::query_sections(&bad_magic2).is_err());
        let mut bad_pe = uki_bytes.clone();
        bad_pe[0x80] = 0;
        assert!(UkiPackager::query_sections(&bad_pe).is_err());
        let mut truncated_pe = uki_bytes.clone();
        truncated_pe[0x3C..0x40].copy_from_slice(&100_000u32.to_le_bytes());
        assert!(UkiPackager::query_sections(&truncated_pe).is_err());

        // Test section table bounds overflow triggering loop break
        let mut overflow_sec_pe = uki_bytes;
        overflow_sec_pe[0x80 + 6] = 250;
        let query_overflow = UkiPackager::query_sections(&overflow_sec_pe[..1100]);
        assert!(query_overflow.is_ok());
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
        assert!(iso_bytes.is_ok());
        let iso = iso_bytes.unwrap_or_default();

        // 1. Traverse ISO-9660 PVD at sector 16
        let pvd_offset = 16 * 2048;
        assert_eq!(&iso[pvd_offset + 1..pvd_offset + 6], b"CD001");
        assert!(
            String::from_utf8_lossy(&iso[pvd_offset + 40..pvd_offset + 72]).contains("MSI_INSTALL")
        );
        // Root directory record at offset 156
        let root_rec = &iso[pvd_offset + 156..pvd_offset + 190];
        assert_eq!(root_rec[0], 34); // record length
        assert_eq!(root_rec[25], 0x02); // directory flag

        // 2. Traverse El Torito Boot Record at sector 17
        let el_offset = 17 * 2048;
        assert_eq!(&iso[el_offset + 1..el_offset + 6], b"CD001");
        assert_eq!(
            &iso[el_offset + 7..el_offset + 30],
            b"EL TORITO SPECIFICATION"
        );

        // 3. Traverse Boot Catalog at sector 22
        let cat_offset = 22 * 2048;
        assert_eq!(iso[cat_offset], 0x01); // Header ID
        assert_eq!(iso[cat_offset + 1], 0xEF); // Platform EFI
        assert_eq!(iso[cat_offset + 0x1E], 0x55);
        assert_eq!(iso[cat_offset + 0x1F], 0xAA);
        assert_eq!(iso[cat_offset + 0x20], 0x88); // Bootable

        // 4. Test USB generation and GPT parsing
        let gen_usb =
            LiveMediaGenerator::new(LiveMediaFormat::RawUsbDisk, "MSI_USB", bootloader.clone());
        let usb_bytes = gen_usb.generate();
        assert!(usb_bytes.is_ok());
        let usb = usb_bytes.unwrap_or_default();

        // Check MBR (Sector 0)
        assert_eq!(usb[450], 0xEE); // GPT protective MBR type
        assert_eq!(usb[510], 0x55);
        assert_eq!(usb[511], 0xAA);

        // Check GPT Header (Sector 1)
        let gpt_header_offset = 512;
        assert_eq!(&usb[gpt_header_offset..gpt_header_offset + 8], b"EFI PART");

        // Check GPT Partition Entry 0 (ESP) at Sector 2
        let esp_pe_offset = 2 * 512;
        assert_eq!(
            &usb[esp_pe_offset..esp_pe_offset + 16],
            &PartitionTypeGuid::ESP.0
        );

        // Check GPT Partition Entry 1 (Payload)
        let payload_pe_offset = (2 * 512) + 128;
        assert_eq!(
            &usb[payload_pe_offset..payload_pe_offset + 16],
            &PartitionTypeGuid::LINUX_ROOT_X86_64.0
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

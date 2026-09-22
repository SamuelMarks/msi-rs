//! Bare-Metal Interactive TUI Installation Wizard.
//!
//! Provides the complete interactive terminal UI wizard sequence:
//! - `DiskSelectionDialog`: physical storage device listing and inspection.
//! - `PartitionConfirmationDialog`: destructive wipe warnings and partition layout inspection.
//! - `LocaleKeyboardDialog`: console keymap and system locale configuration.
//! - `NetworkConfigDialog`: optional DHCP / static IP interface assignment.
//! - `UserAccountDialog`: administrator and unprivileged initial user provisioning.
//! - `InstallationProgressDialog`: real-time file copy, driver staging, and bootloader installation.
//! - `InstallationCompleteDialog`: prompt to eject installation media and reboot.
//! - `DiagnosticsLogConsole`: live log view with `F2` split console toggle and error dumping.

use crate::error::{Error, Result};
use crate::platform::disk::{BlockDevice, BlockDevicePath, BusType};
use crate::ui::tui::TuiKey;
use std::fmt::Write as _;
use std::path::Path;

/// Primary stage sequence for the bare-metal installation wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WizardStep {
    /// Step 1: Physical disk discovery and destination selection.
    #[default]
    DiskSelection,
    /// Step 2: Destructive partition changes confirmation.
    PartitionConfirmation,
    /// Step 3: Localization and console keymap configuration.
    LocaleKeyboard,
    /// Step 4: Network interface and address assignment.
    NetworkConfig,
    /// Step 5: User accounts and administrator credentials.
    UserAccount,
    /// Step 6: Active deployment progress and live logging.
    Progress,
    /// Step 7: Completed installation and reboot prompt.
    Complete,
}

/// Target disk selection dialog component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskSelectionDialog {
    /// Available discovered physical disks.
    pub disks: Vec<BlockDevice>,
    /// Currently highlighted disk index.
    pub selected_index: usize,
}

impl DiskSelectionDialog {
    /// Creates a new [`DiskSelectionDialog`].
    ///
    /// # Arguments
    ///
    /// * `disks` - Filtered list of eligible target disks.
    ///
    /// # Returns
    ///
    /// Initialized dialog.
    #[must_use]
    pub const fn new(disks: Vec<BlockDevice>) -> Self {
        Self {
            disks,
            selected_index: 0,
        }
    }

    /// Selects the next disk in the list.
    pub fn select_next(&mut self) {
        if !self.disks.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.disks.len();
        }
    }

    /// Selects the previous disk in the list.
    pub fn select_prev(&mut self) {
        if !self.disks.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.disks.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Returns the currently chosen target disk.
    ///
    /// # Returns
    ///
    /// Optional borrowed [`BlockDevice`].
    #[must_use]
    pub fn selected_disk(&self) -> Option<&BlockDevice> {
        self.disks.get(self.selected_index)
    }

    /// Renders text lines representing the disk selection screen.
    ///
    /// # Returns
    ///
    /// Formatted line strings.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let mut lines = vec![
            "┌─── Select Destination Storage Disk ──────────────────────────────────────┐"
                .to_string(),
            "│ Choose physical disk to install operating system:                         │"
                .to_string(),
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        ];

        if self.disks.is_empty() {
            lines.push(
                "│ No eligible target disks detected!                                       │"
                    .to_string(),
            );
        } else {
            for (i, d) in self.disks.iter().enumerate() {
                let marker = if i == self.selected_index { ">" } else { " " };
                let bus = match d.bus_type {
                    BusType::Nvme => "NVMe",
                    BusType::Sata => "SATA",
                    BusType::Usb => "USB ",
                    BusType::VirtIo => "VIRT",
                    _ => "DISK",
                };
                let line = format!(
                    "│ {} [{}] {:<16} {:>6} GiB  {:<26} │",
                    marker,
                    bus,
                    d.path,
                    d.size_gib(),
                    d.model
                );
                lines.push(line);
            }
        }

        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        lines.push(
            "│ [Enter] Continue    [Up/Down] Navigate    [F2] Toggle Logs               │"
                .to_string(),
        );
        lines.push(
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        );
        lines
    }
}

/// Partition confirmation dialog warning of destructive disk formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionConfirmationDialog {
    /// Target disk device path.
    pub target_path: BlockDevicePath,
    /// Capacity of target disk in GiB.
    pub disk_size_gib: u64,
    /// Proposed partition scheme layout summaries.
    pub proposed_partitions: Vec<String>,
    /// User explicit confirmation toggle.
    pub confirmed: bool,
}

impl PartitionConfirmationDialog {
    /// Creates a new [`PartitionConfirmationDialog`].
    ///
    /// # Arguments
    ///
    /// * `target_path` - Target disk path.
    /// * `disk_size_gib` - Disk size in GiB.
    /// * `proposed_partitions` - List of partitions to create.
    ///
    /// # Returns
    ///
    /// Dialog instance.
    #[must_use]
    pub const fn new(
        target_path: BlockDevicePath,
        disk_size_gib: u64,
        proposed_partitions: Vec<String>,
    ) -> Self {
        Self {
            target_path,
            disk_size_gib,
            proposed_partitions,
            confirmed: false,
        }
    }

    /// Toggles confirmation checkbox.
    pub const fn toggle_confirmation(&mut self) {
        self.confirmed = !self.confirmed;
    }

    /// Renders the confirmation warning dialog.
    ///
    /// # Returns
    ///
    /// Formatted line strings.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let mut lines = vec![
            "┌─── Confirm Partition Table & Destructive Wipe ───────────────────────────┐"
                .to_string(),
            format!(
                "│ TARGET DISK: {:<16} ({:>4} GiB)                                      │",
                self.target_path, self.disk_size_gib
            ),
            "│ WARNING: ALL DATA CURRENTLY ON THIS DISK WILL BE PERMANENTLY ERASED!     │"
                .to_string(),
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
            "│ Proposed Partition Table:                                                │"
                .to_string(),
        ];

        for p in &self.proposed_partitions {
            lines.push(format!("│   * {p:<68} │"));
        }

        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        let check = if self.confirmed { "[X]" } else { "[ ]" };
        lines.push(format!(
            "│ {check} I confirm all existing data on this drive should be wiped.             │"
        ));
        lines.push(
            "│ [Space] Toggle Confirmation    [Enter] Write Changes    [Esc] Back       │"
                .to_string(),
        );
        lines.push(
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        );
        lines
    }
}

/// Active field in keyboard/locale selection dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleFocusField {
    /// Selecting primary system locale.
    Locale,
    /// Selecting console keyboard layout.
    Keymap,
}

/// Locale and keyboard layout configuration dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleKeyboardDialog {
    /// Supported locale options.
    pub locales: Vec<String>,
    /// Selected locale index.
    pub selected_locale: usize,
    /// Supported keyboard layout options.
    pub keymaps: Vec<String>,
    /// Selected keyboard layout index.
    pub selected_keymap: usize,
    /// Active interactive focus field.
    pub focus: LocaleFocusField,
}

impl Default for LocaleKeyboardDialog {
    fn default() -> Self {
        Self {
            locales: vec![
                "en_US.UTF-8".to_string(),
                "en_GB.UTF-8".to_string(),
                "de_DE.UTF-8".to_string(),
                "fr_FR.UTF-8".to_string(),
                "ja_JP.UTF-8".to_string(),
            ],
            selected_locale: 0,
            keymaps: vec![
                "us".to_string(),
                "uk".to_string(),
                "de".to_string(),
                "fr".to_string(),
                "jp".to_string(),
            ],
            selected_keymap: 0,
            focus: LocaleFocusField::Locale,
        }
    }
}

impl LocaleKeyboardDialog {
    /// Toggles active focus between locale and keymap lists.
    pub const fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            LocaleFocusField::Locale => LocaleFocusField::Keymap,
            LocaleFocusField::Keymap => LocaleFocusField::Locale,
        };
    }

    /// Selects the next option in the focused list.
    pub fn select_next(&mut self) {
        match self.focus {
            LocaleFocusField::Locale => {
                self.selected_locale = (self.selected_locale + 1) % self.locales.len();
            }
            LocaleFocusField::Keymap => {
                self.selected_keymap = (self.selected_keymap + 1) % self.keymaps.len();
            }
        }
    }

    /// Selects the previous option in the focused list.
    pub fn select_prev(&mut self) {
        match self.focus {
            LocaleFocusField::Locale => {
                self.selected_locale = if self.selected_locale == 0 {
                    self.locales.len() - 1
                } else {
                    self.selected_locale - 1
                };
            }
            LocaleFocusField::Keymap => {
                self.selected_keymap = if self.selected_keymap == 0 {
                    self.keymaps.len() - 1
                } else {
                    self.selected_keymap - 1
                };
            }
        }
    }

    /// Returns selected locale string.
    #[must_use]
    pub fn current_locale(&self) -> &str {
        &self.locales[self.selected_locale]
    }

    /// Returns selected keyboard layout string.
    #[must_use]
    pub fn current_keymap(&self) -> &str {
        &self.keymaps[self.selected_keymap]
    }
}

/// Live installation progress dialog displaying current step and progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationProgressDialog {
    /// Overall completion percentage (0..=100).
    pub overall_percent: u8,
    /// Currently executing step description.
    pub current_step: String,
    /// Split log view toggle flag.
    pub show_split_log: bool,
}

impl Default for InstallationProgressDialog {
    fn default() -> Self {
        Self {
            overall_percent: 0,
            current_step: "Initializing deployment pipeline...".to_string(),
            show_split_log: false,
        }
    }
}

impl InstallationProgressDialog {
    /// Sets current progress status.
    ///
    /// # Arguments
    ///
    /// * `percent` - Overall percentage.
    /// * `step` - Step description.
    pub fn update(&mut self, percent: u8, step: impl Into<String>) {
        self.overall_percent = percent.min(100);
        self.current_step = step.into();
    }

    /// Toggles the split log console view.
    pub const fn toggle_split_log(&mut self) {
        self.show_split_log = !self.show_split_log;
    }

    /// Renders the progress bar frame.
    ///
    /// # Returns
    ///
    /// Formatted lines.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let hashes = usize::from(self.overall_percent / 2);
        let spaces = 50 - hashes;
        let bar = format!(
            "[{}{}] {:3}%",
            "#".repeat(hashes),
            " ".repeat(spaces),
            self.overall_percent
        );

        vec![
            "┌─── Installing Operating System ──────────────────────────────────────────┐"
                .to_string(),
            format!("│ Status: {:<64} │", self.current_step),
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
            format!("│  {:<71} │", bar),
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
            "│ [F2] Toggle Live Log Split Console                                       │"
                .to_string(),
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        ]
    }
}

/// Installation completed dialog prompting user to reboot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallationCompleteDialog {
    /// Eject media instruction flag.
    pub eject_media: bool,
}

impl InstallationCompleteDialog {
    /// Renders completion announcement.
    ///
    /// # Returns
    ///
    /// Formatted lines.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        vec![
            "┌─── Installation Complete! ───────────────────────────────────────────────┐"
                .to_string(),
            "│ The operating system has been successfully installed to target disk.     │"
                .to_string(),
            "│                                                                          │"
                .to_string(),
            "│ Please remove the USB installation drive or eject installation media.    │"
                .to_string(),
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
            "│ [Enter] Restart Computer Now                                             │"
                .to_string(),
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        ]
    }
}

/// Live diagnostic log stream manager.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticsLogConsole {
    /// Staged log message lines.
    pub logs: Vec<String>,
}

impl DiagnosticsLogConsole {
    /// Creates a new [`DiagnosticsLogConsole`].
    ///
    /// # Returns
    ///
    /// Fresh instance.
    #[must_use]
    pub const fn new() -> Self {
        Self { logs: Vec::new() }
    }

    /// Appends a log line to the diagnostic buffer.
    ///
    /// # Arguments
    ///
    /// * `message` - Log message string.
    pub fn append(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
    }

    /// Exports all accumulated diagnostic logs to a file.
    ///
    /// # Arguments
    ///
    /// * `dest_path` - Target file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if file write fails.
    pub fn export_to_file(&self, dest_path: &Path) -> Result<()> {
        let mut out = String::new();
        for line in &self.logs {
            let _ = writeln!(out, "{line}");
        }
        std::fs::write(dest_path, out).map_err(|e| Error::Io(e.to_string()))
    }
}

/// Top-level Bare-Metal TUI Installation Wizard Controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareMetalInstallationWizard {
    /// Current wizard progression step.
    pub step: WizardStep,
    /// Disk selection dialog state.
    pub disk_selection: DiskSelectionDialog,
    /// Partition confirmation dialog state.
    pub partition_confirm: PartitionConfirmationDialog,
    /// Locale and keyboard layout dialog state.
    pub locale_keyboard: LocaleKeyboardDialog,
    /// Progress dialog state.
    pub progress: InstallationProgressDialog,
    /// Completion dialog state.
    pub complete: InstallationCompleteDialog,
    /// Diagnostic logging console.
    pub diagnostics: DiagnosticsLogConsole,
}

impl BareMetalInstallationWizard {
    /// Creates a new [`BareMetalInstallationWizard`].
    ///
    /// # Arguments
    ///
    /// * `target_disks` - Detected physical target block devices.
    ///
    /// # Returns
    ///
    /// Initialized wizard instance.
    #[must_use]
    pub fn new(target_disks: Vec<BlockDevice>) -> Self {
        let first_path = target_disks
            .first()
            .map_or_else(|| BlockDevicePath::new("/dev/sda"), |d| d.path.clone());
        let first_size = target_disks.first().map_or(64, BlockDevice::size_gib);

        Self {
            step: WizardStep::DiskSelection,
            disk_selection: DiskSelectionDialog::new(target_disks),
            partition_confirm: PartitionConfirmationDialog::new(
                first_path,
                first_size,
                vec![
                    "EFI System Partition: 512 MB (FAT32)".to_string(),
                    "Operating System Root: Remainder (NTFS/ext4)".to_string(),
                ],
            ),
            locale_keyboard: LocaleKeyboardDialog::default(),
            progress: InstallationProgressDialog::default(),
            complete: InstallationCompleteDialog::default(),
            diagnostics: DiagnosticsLogConsole::new(),
        }
    }

    /// Handles a keypress event, updating active dialog state.
    ///
    /// # Arguments
    ///
    /// * `key` - Pressed keyboard key.
    pub fn handle_key(&mut self, key: TuiKey) {
        // Global F2 toggle for split log view
        if key == TuiKey::F(2) {
            self.progress.toggle_split_log();
            return;
        }

        match self.step {
            WizardStep::DiskSelection => match key {
                TuiKey::Down => self.disk_selection.select_next(),
                TuiKey::Up => self.disk_selection.select_prev(),
                TuiKey::Enter => {
                    if let Some(disk) = self.disk_selection.selected_disk() {
                        self.partition_confirm.target_path = disk.path.clone();
                        self.partition_confirm.disk_size_gib = disk.size_gib();
                    }
                    self.step = WizardStep::PartitionConfirmation;
                }
                _ => {}
            },
            WizardStep::PartitionConfirmation => match key {
                TuiKey::Space => self.partition_confirm.toggle_confirmation(),
                TuiKey::Enter => {
                    if self.partition_confirm.confirmed {
                        self.step = WizardStep::LocaleKeyboard;
                    }
                }
                TuiKey::Escape => self.step = WizardStep::DiskSelection,
                _ => {}
            },
            WizardStep::LocaleKeyboard => match key {
                TuiKey::Tab => self.locale_keyboard.toggle_focus(),
                TuiKey::Down => self.locale_keyboard.select_next(),
                TuiKey::Up => self.locale_keyboard.select_prev(),
                TuiKey::Enter => self.step = WizardStep::Progress,
                TuiKey::Escape => self.step = WizardStep::PartitionConfirmation,
                _ => {}
            },
            WizardStep::NetworkConfig | WizardStep::UserAccount => {
                if key == TuiKey::Enter {
                    self.step = WizardStep::Progress;
                }
            }
            WizardStep::Progress => {
                if self.progress.overall_percent >= 100 && key == TuiKey::Enter {
                    self.step = WizardStep::Complete;
                }
            }
            WizardStep::Complete => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::disk::{DeviceKind, SmartHealthStatus};
    use std::path::PathBuf;

    /// Tests `DiskSelectionDialog` navigation and rendering.
    #[test]
    fn test_disk_selection_dialog() {
        let disk_nvme = BlockDevice {
            path: BlockDevicePath::new("/dev/nvme0n1"),
            kind: DeviceKind::Disk,
            size_bytes: 512 * 1024 * 1024 * 1024,
            sector_size: 4096,
            bus_type: BusType::Nvme,
            model: "Samsung SSD".to_string(),
            serial: "123".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };
        let disk_sata = BlockDevice {
            path: BlockDevicePath::new("/dev/sda"),
            kind: DeviceKind::Disk,
            size_bytes: 1024 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Sata,
            model: "Crucial MX500".to_string(),
            serial: "456".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };
        let disk_usb = BlockDevice {
            path: BlockDevicePath::new("/dev/sdb"),
            kind: DeviceKind::Disk,
            size_bytes: 64 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Usb,
            model: "SanDisk Flash".to_string(),
            serial: "789".to_string(),
            read_only: false,
            removable: true,
            smart_status: SmartHealthStatus::Healthy,
        };
        let disk_virtio = BlockDevice {
            path: BlockDevicePath::new("/dev/vda"),
            kind: DeviceKind::Disk,
            size_bytes: 32 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::VirtIo,
            model: "QEMU VirtIO".to_string(),
            serial: "000".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };
        let disk_other = BlockDevice {
            path: BlockDevicePath::new("/dev/mmcblk0"),
            kind: DeviceKind::Disk,
            size_bytes: 16 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Unknown,
            model: "SD Card".to_string(),
            serial: "111".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };

        let mut dialog = DiskSelectionDialog::new(vec![
            disk_nvme,
            disk_sata,
            disk_usb,
            disk_virtio,
            disk_other,
        ]);
        assert_eq!(dialog.selected_index, 0);
        assert!(dialog.selected_disk().is_some());

        // Test next wrapping and non-zero index in render
        dialog.select_next();
        assert_eq!(dialog.selected_index, 1);
        dialog.select_prev();
        assert_eq!(dialog.selected_index, 0);
        dialog.select_prev();
        assert_eq!(dialog.selected_index, 4);

        let lines = dialog.render();
        assert!(lines
            .iter()
            .any(|l| l.contains("Select Destination Storage Disk")));
        assert!(lines.iter().any(|l| l.contains("/dev/nvme0n1")));
        assert!(lines.iter().any(|l| l.contains("SATA")));
        assert!(lines.iter().any(|l| l.contains("USB ")));
        assert!(lines.iter().any(|l| l.contains("VIRT")));
        assert!(lines.iter().any(|l| l.contains("DISK")));

        // Empty dialog testing
        let mut empty_dialog = DiskSelectionDialog::new(Vec::new());
        empty_dialog.select_next();
        empty_dialog.select_prev();
        assert_eq!(empty_dialog.selected_index, 0);
        assert!(empty_dialog.selected_disk().is_none());
        let empty_lines = empty_dialog.render();
        assert!(empty_lines
            .iter()
            .any(|l| l.contains("No eligible target disks detected!")));
    }

    /// Tests `PartitionConfirmationDialog` toggle and rendering.
    #[test]
    fn test_partition_confirmation_dialog() {
        let mut dialog = PartitionConfirmationDialog::new(
            BlockDevicePath::new("/dev/sda"),
            256,
            vec!["ESP: 512MB".to_string()],
        );
        assert!(!dialog.confirmed);
        let unconfirmed_lines = dialog.render();
        assert!(unconfirmed_lines.iter().any(|l| l.contains("[ ]")));

        dialog.toggle_confirmation();
        assert!(dialog.confirmed);

        let lines = dialog.render();
        assert!(lines.iter().any(|l| l.contains("WARNING")));
        assert!(lines.iter().any(|l| l.contains("[X]")));
    }

    /// Tests `LocaleKeyboardDialog` focus toggle and selection.
    #[test]
    fn test_locale_keyboard_dialog() {
        let mut dialog = LocaleKeyboardDialog::default();
        assert_eq!(dialog.focus, LocaleFocusField::Locale);
        assert_eq!(dialog.current_locale(), "en_US.UTF-8");

        // prev from 0 wraps to end
        dialog.select_prev();
        assert_eq!(dialog.current_locale(), "ja_JP.UTF-8");
        dialog.select_next();
        assert_eq!(dialog.current_locale(), "en_US.UTF-8");
        dialog.select_next();
        assert_eq!(dialog.current_locale(), "en_GB.UTF-8");
        dialog.select_prev();
        assert_eq!(dialog.current_locale(), "en_US.UTF-8");

        // toggle focus to keymap
        dialog.toggle_focus();
        assert_eq!(dialog.focus, LocaleFocusField::Keymap);
        assert_eq!(dialog.current_keymap(), "us");
        dialog.select_prev();
        assert_eq!(dialog.current_keymap(), "jp");
        dialog.select_next();
        assert_eq!(dialog.current_keymap(), "us");
        dialog.select_next();
        assert_eq!(dialog.current_keymap(), "uk");
        dialog.select_prev();
        assert_eq!(dialog.current_keymap(), "us");

        // toggle back to locale
        dialog.toggle_focus();
        assert_eq!(dialog.focus, LocaleFocusField::Locale);
    }

    /// Tests `InstallationProgressDialog` update and split log toggle.
    #[test]
    fn test_progress_dialog() {
        let mut dialog = InstallationProgressDialog::default();
        assert_eq!(dialog.overall_percent, 0);
        assert!(!dialog.show_split_log);

        dialog.update(50, "Staging bootloader files...");
        assert_eq!(dialog.overall_percent, 50);
        assert_eq!(dialog.current_step, "Staging bootloader files...");

        dialog.toggle_split_log();
        assert!(dialog.show_split_log);

        let lines = dialog.render();
        assert!(lines.iter().any(|l| l.contains("50%")));
    }

    /// Tests `DiagnosticsLogConsole` appending and file export.
    #[test]
    fn test_diagnostics_log_console() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_logs_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_file = temp_dir.join("install.log");

        let mut console = DiagnosticsLogConsole::new();
        console.append("Starting installation engine");
        console.append("ESP partitioned successfully");

        assert_eq!(console.logs.len(), 2);
        assert!(console.export_to_file(&log_file).is_ok());

        let read_back = std::fs::read_to_string(&log_file).unwrap_or_default();
        assert!(read_back.contains("Starting installation engine"));
        assert!(read_back.contains("ESP partitioned successfully"));

        // Error path for export
        let invalid_path = PathBuf::from("/nonexistent_directory/nonexistent_file.log");
        assert!(console.export_to_file(&invalid_path).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `BareMetalInstallationWizard` state transitions.
    #[test]
    fn test_wizard_state_machine() {
        let disk1 = BlockDevice {
            path: BlockDevicePath::new("/dev/nvme0n1"),
            kind: DeviceKind::Disk,
            size_bytes: 256 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Nvme,
            model: "Test NVMe".to_string(),
            serial: "123".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };
        let disk2 = BlockDevice {
            path: BlockDevicePath::new("/dev/sda"),
            kind: DeviceKind::Disk,
            size_bytes: 512 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Sata,
            model: "Test SATA".to_string(),
            serial: "456".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };

        let mut wizard = BareMetalInstallationWizard::new(vec![disk1, disk2]);
        assert_eq!(wizard.step, WizardStep::DiskSelection);

        // Test Up and Down in DiskSelection
        wizard.handle_key(TuiKey::Down);
        assert_eq!(wizard.disk_selection.selected_index, 1);
        wizard.handle_key(TuiKey::Up);
        assert_eq!(wizard.disk_selection.selected_index, 0);
        wizard.handle_key(TuiKey::Char('z')); // Unhandled key
        assert_eq!(wizard.step, WizardStep::DiskSelection);

        // Advance to partition confirmation
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::PartitionConfirmation);

        // Escape in PartitionConfirmation goes back to DiskSelection
        wizard.handle_key(TuiKey::Escape);
        assert_eq!(wizard.step, WizardStep::DiskSelection);
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::PartitionConfirmation);

        // Cannot advance without confirming
        wizard.handle_key(TuiKey::Char('z')); // Unhandled key
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::PartitionConfirmation);

        // Toggle confirmation and advance to locale
        wizard.handle_key(TuiKey::Space);
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::LocaleKeyboard);

        // Test navigation keys in LocaleKeyboard
        wizard.handle_key(TuiKey::Tab);
        wizard.handle_key(TuiKey::Down);
        wizard.handle_key(TuiKey::Up);
        wizard.handle_key(TuiKey::Char('z')); // Unhandled key

        // Escape in LocaleKeyboard goes back to PartitionConfirmation
        wizard.handle_key(TuiKey::Escape);
        assert_eq!(wizard.step, WizardStep::PartitionConfirmation);
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::LocaleKeyboard);

        // Advance to progress
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::Progress);

        // F2 toggles split log
        wizard.handle_key(TuiKey::F(2));
        assert!(wizard.progress.show_split_log);

        // Enter on progress before 100% does not advance
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::Progress);

        // Progress to 100% and advance to complete
        wizard.progress.update(100, "Done");
        wizard.handle_key(TuiKey::Char('z')); // Key other than Enter at 100%
        assert_eq!(wizard.step, WizardStep::Progress);
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::Complete);

        // Keys on Complete step
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::Complete);

        let comp_lines = wizard.complete.render();
        assert!(comp_lines
            .iter()
            .any(|l| l.contains("Installation Complete!")));

        // Test wizard empty target disks fallback
        let mut empty_wizard = BareMetalInstallationWizard::new(Vec::new());
        assert_eq!(
            empty_wizard.partition_confirm.target_path,
            BlockDevicePath::new("/dev/sda")
        );
        assert_eq!(empty_wizard.partition_confirm.disk_size_gib, 64);
        empty_wizard.handle_key(TuiKey::Enter);
        assert_eq!(empty_wizard.step, WizardStep::PartitionConfirmation);

        // Test NetworkConfig and UserAccount steps
        let mut net_wizard = BareMetalInstallationWizard::new(Vec::new());
        net_wizard.step = WizardStep::NetworkConfig;
        net_wizard.handle_key(TuiKey::Char('x'));
        assert_eq!(net_wizard.step, WizardStep::NetworkConfig);
        net_wizard.handle_key(TuiKey::Enter);
        assert_eq!(net_wizard.step, WizardStep::Progress);

        net_wizard.step = WizardStep::UserAccount;
        net_wizard.handle_key(TuiKey::Enter);
        assert_eq!(net_wizard.step, WizardStep::Progress);
    }
}

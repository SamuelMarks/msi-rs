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

    /// Renders text lines representing the locale and keyboard configuration dialog.
    ///
    /// # Returns
    ///
    /// Formatted ASCII line strings.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(
            "┌──────────────────────────────────────────────────────────────────────────┐"
                .to_string(),
        );
        lines.push(
            "│ Configure System Locale & Keyboard Layout                                │"
                .to_string(),
        );
        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        let loc_cursor = if self.focus == LocaleFocusField::Locale {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {loc_cursor} System Locale: {:<58} │",
            self.current_locale()
        ));
        let key_cursor = if self.focus == LocaleFocusField::Keymap {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {key_cursor} Keyboard Map:  {:<58} │",
            self.current_keymap()
        ));
        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        lines.push(
            "│ [Tab] Toggle Focus    [Up/Down] Select Option    [Enter] Next   [Esc] Back│"
                .to_string(),
        );
        lines.push(
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        );
        lines
    }
}

/// Network configuration operational mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkConfigMode {
    /// Automatic network assignment via DHCP.
    #[default]
    Dhcp,
    /// Manual static IP, netmask, gateway, and DNS.
    Static,
    /// Skip network setup during deployment.
    Skip,
}

/// Focused interactive field in network configuration dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkFocusField {
    /// Selecting target network interface.
    #[default]
    Interface,
    /// Selecting network configuration mode (DHCP / Static / Skip).
    Mode,
    /// Entering static IP address.
    IpAddress,
    /// Entering subnet mask or CIDR.
    SubnetMask,
    /// Entering default gateway IP.
    Gateway,
    /// Entering DNS server IP.
    DnsServer,
}

/// Network interface and address assignment configuration dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConfigDialog {
    /// Detected or configured network interfaces.
    pub interfaces: Vec<String>,
    /// Selected interface index.
    pub selected_interface: usize,
    /// Network assignment mode.
    pub mode: NetworkConfigMode,
    /// Static IPv4/IPv6 address string.
    pub ip_address: String,
    /// Static subnet mask string.
    pub subnet_mask: String,
    /// Default gateway router address.
    pub gateway: String,
    /// Configured DNS server addresses.
    pub dns_servers: Vec<String>,
    /// Currently focused interactive field.
    pub focus: NetworkFocusField,
    /// Validation error message if last validation failed.
    pub error_message: Option<String>,
}

impl Default for NetworkConfigDialog {
    fn default() -> Self {
        Self {
            interfaces: vec!["eth0".to_string(), "wlan0".to_string()],
            selected_interface: 0,
            mode: NetworkConfigMode::Dhcp,
            ip_address: "192.168.1.100".to_string(),
            subnet_mask: "255.255.255.0".to_string(),
            gateway: "192.168.1.1".to_string(),
            dns_servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
            focus: NetworkFocusField::Interface,
            error_message: None,
        }
    }
}

impl NetworkConfigDialog {
    /// Creates a new [`NetworkConfigDialog`] with detected network interfaces.
    ///
    /// # Arguments
    ///
    /// * `interfaces` - List of detected network interface device names.
    ///
    /// # Returns
    ///
    /// Initialized dialog instance.
    #[must_use]
    pub fn new(interfaces: Vec<String>) -> Self {
        Self {
            interfaces,
            ..Self::default()
        }
    }

    /// Cycles through network configuration modes.
    pub const fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            NetworkConfigMode::Dhcp => NetworkConfigMode::Static,
            NetworkConfigMode::Static => NetworkConfigMode::Skip,
            NetworkConfigMode::Skip => NetworkConfigMode::Dhcp,
        };
    }

    /// Advances active focus to the next field in sequence.
    pub const fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            NetworkFocusField::Interface => NetworkFocusField::Mode,
            NetworkFocusField::Mode => NetworkFocusField::IpAddress,
            NetworkFocusField::IpAddress => NetworkFocusField::SubnetMask,
            NetworkFocusField::SubnetMask => NetworkFocusField::Gateway,
            NetworkFocusField::Gateway => NetworkFocusField::DnsServer,
            NetworkFocusField::DnsServer => NetworkFocusField::Interface,
        };
    }

    /// Moves active focus to the previous field in sequence.
    pub const fn focus_prev(&mut self) {
        self.focus = match self.focus {
            NetworkFocusField::Interface => NetworkFocusField::DnsServer,
            NetworkFocusField::Mode => NetworkFocusField::Interface,
            NetworkFocusField::IpAddress => NetworkFocusField::Mode,
            NetworkFocusField::SubnetMask => NetworkFocusField::IpAddress,
            NetworkFocusField::Gateway => NetworkFocusField::SubnetMask,
            NetworkFocusField::DnsServer => NetworkFocusField::Gateway,
        };
    }

    /// Selects the next available network interface.
    pub fn select_next_interface(&mut self) {
        if !self.interfaces.is_empty() {
            self.selected_interface = (self.selected_interface + 1) % self.interfaces.len();
        }
    }

    /// Appends a typed character to the currently focused text field.
    ///
    /// # Arguments
    ///
    /// * `c` - Input character.
    pub fn handle_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        match self.focus {
            NetworkFocusField::IpAddress => self.ip_address.push(c),
            NetworkFocusField::SubnetMask => self.subnet_mask.push(c),
            NetworkFocusField::Gateway => self.gateway.push(c),
            NetworkFocusField::DnsServer => {
                if let Some(first_dns) = self.dns_servers.first_mut() {
                    first_dns.push(c);
                } else {
                    self.dns_servers.push(c.to_string());
                }
            }
            NetworkFocusField::Interface | NetworkFocusField::Mode => {}
        }
    }

    /// Removes the trailing character from the currently focused text field.
    pub fn handle_backspace(&mut self) {
        match self.focus {
            NetworkFocusField::IpAddress => {
                self.ip_address.pop();
            }
            NetworkFocusField::SubnetMask => {
                self.subnet_mask.pop();
            }
            NetworkFocusField::Gateway => {
                self.gateway.pop();
            }
            NetworkFocusField::DnsServer => {
                if let Some(first_dns) = self.dns_servers.first_mut() {
                    first_dns.pop();
                }
            }
            NetworkFocusField::Interface | NetworkFocusField::Mode => {}
        }
    }

    /// Validates network configuration fields, updating error message state.
    ///
    /// # Returns
    ///
    /// `true` if configuration is valid.
    pub fn validate(&mut self) -> bool {
        if self.mode == NetworkConfigMode::Static {
            if self.ip_address.parse::<std::net::IpAddr>().is_err() {
                self.error_message = Some("Invalid static IP address syntax".to_string());
                return false;
            }
            if self.subnet_mask.parse::<std::net::IpAddr>().is_err() {
                self.error_message = Some("Invalid subnet mask syntax".to_string());
                return false;
            }
            if !self.gateway.is_empty() && self.gateway.parse::<std::net::IpAddr>().is_err() {
                self.error_message = Some("Invalid gateway IP address syntax".to_string());
                return false;
            }
        }
        self.error_message = None;
        true
    }

    /// Renders text lines representing the network configuration dialog.
    ///
    /// # Returns
    ///
    /// Formatted ASCII line strings.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(
            "┌──────────────────────────────────────────────────────────────────────────┐"
                .to_string(),
        );
        lines.push(
            "│ Network Interface & Address Assignment                                   │"
                .to_string(),
        );
        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        let iface = self
            .interfaces
            .get(self.selected_interface)
            .map_or("none", String::as_str);
        let iface_c = if self.focus == NetworkFocusField::Interface {
            ">"
        } else {
            " "
        };
        lines.push(format!("│ {iface_c} Interface:   {iface:<59} │"));
        let mode_c = if self.focus == NetworkFocusField::Mode {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {mode_c} Mode:        {:<59} │",
            format!("{:?}", self.mode)
        ));
        let ip_c = if self.focus == NetworkFocusField::IpAddress {
            ">"
        } else {
            " "
        };
        lines.push(format!("│ {ip_c} IP Address:  {:<59} │", self.ip_address));
        let mask_c = if self.focus == NetworkFocusField::SubnetMask {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {mask_c} Subnet Mask: {:<59} │",
            self.subnet_mask
        ));
        let gw_c = if self.focus == NetworkFocusField::Gateway {
            ">"
        } else {
            " "
        };
        lines.push(format!("│ {gw_c} Gateway:     {:<59} │", self.gateway));
        let dns_c = if self.focus == NetworkFocusField::DnsServer {
            ">"
        } else {
            " "
        };
        let dns_str = self.dns_servers.join(", ");
        lines.push(format!("│ {dns_c} DNS Servers: {dns_str:<59} │"));
        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        if let Some(ref err) = self.error_message {
            lines.push(format!("│ [!] ERROR: {err:<61} │"));
        } else {
            lines.push(
                "│ Ready to configure network parameters.                                   │"
                    .to_string(),
            );
        }
        lines.push(
            "│ [Tab] Next Field  [Space] Cycle Option  [Enter] Next  [Esc] Back         │"
                .to_string(),
        );
        lines.push(
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        );
        lines
    }
}

/// Focused field in user account and credentials dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserAccountFocusField {
    /// Entering root/administrator password.
    #[default]
    RootPassword,
    /// Entering root/administrator password confirmation.
    RootPasswordConfirm,
    /// Entering initial unprivileged username.
    Username,
    /// Entering initial user password.
    UserPassword,
    /// Entering initial user password confirmation.
    UserPasswordConfirm,
    /// Toggling sudo / administrator privileges.
    GrantSudo,
}

/// User accounts and administrator credential provisioning dialog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserAccountDialog {
    /// System root / administrator password.
    pub root_password: String,
    /// Root password confirmation.
    pub root_password_confirm: String,
    /// Initial non-root username.
    pub username: String,
    /// Initial user password.
    pub user_password: String,
    /// User password confirmation.
    pub user_password_confirm: String,
    /// Grant sudo / wheel / administrator group membership.
    pub grant_sudo: bool,
    /// Currently focused interactive field.
    pub focus: UserAccountFocusField,
    /// Validation error message.
    pub error_message: Option<String>,
}

impl UserAccountDialog {
    /// Creates a new [`UserAccountDialog`] with default initial configuration.
    ///
    /// # Returns
    ///
    /// Default dialog instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            username: "admin".to_string(),
            grant_sudo: true,
            ..Self::default()
        }
    }

    /// Advances active focus to the next field in sequence.
    pub const fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            UserAccountFocusField::RootPassword => UserAccountFocusField::RootPasswordConfirm,
            UserAccountFocusField::RootPasswordConfirm => UserAccountFocusField::Username,
            UserAccountFocusField::Username => UserAccountFocusField::UserPassword,
            UserAccountFocusField::UserPassword => UserAccountFocusField::UserPasswordConfirm,
            UserAccountFocusField::UserPasswordConfirm => UserAccountFocusField::GrantSudo,
            UserAccountFocusField::GrantSudo => UserAccountFocusField::RootPassword,
        };
    }

    /// Moves active focus to the previous field in sequence.
    pub const fn focus_prev(&mut self) {
        self.focus = match self.focus {
            UserAccountFocusField::RootPassword => UserAccountFocusField::GrantSudo,
            UserAccountFocusField::RootPasswordConfirm => UserAccountFocusField::RootPassword,
            UserAccountFocusField::Username => UserAccountFocusField::RootPasswordConfirm,
            UserAccountFocusField::UserPassword => UserAccountFocusField::Username,
            UserAccountFocusField::UserPasswordConfirm => UserAccountFocusField::UserPassword,
            UserAccountFocusField::GrantSudo => UserAccountFocusField::UserPasswordConfirm,
        };
    }

    /// Toggles administrator / sudo privileges for the initial user account.
    pub const fn toggle_sudo(&mut self) {
        self.grant_sudo = !self.grant_sudo;
    }

    /// Appends a typed character to the currently focused text field.
    ///
    /// # Arguments
    ///
    /// * `c` - Input character.
    pub fn handle_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        match self.focus {
            UserAccountFocusField::RootPassword => self.root_password.push(c),
            UserAccountFocusField::RootPasswordConfirm => self.root_password_confirm.push(c),
            UserAccountFocusField::Username => self.username.push(c),
            UserAccountFocusField::UserPassword => self.user_password.push(c),
            UserAccountFocusField::UserPasswordConfirm => self.user_password_confirm.push(c),
            UserAccountFocusField::GrantSudo => {}
        }
    }

    /// Removes the trailing character from the currently focused text field.
    pub fn handle_backspace(&mut self) {
        match self.focus {
            UserAccountFocusField::RootPassword => {
                self.root_password.pop();
            }
            UserAccountFocusField::RootPasswordConfirm => {
                self.root_password_confirm.pop();
            }
            UserAccountFocusField::Username => {
                self.username.pop();
            }
            UserAccountFocusField::UserPassword => {
                self.user_password.pop();
            }
            UserAccountFocusField::UserPasswordConfirm => {
                self.user_password_confirm.pop();
            }
            UserAccountFocusField::GrantSudo => {}
        }
    }

    /// Validates user account parameters, verifying minimum length and matching confirmations.
    ///
    /// # Returns
    ///
    /// `true` if credentials pass validation.
    pub fn validate(&mut self) -> bool {
        if self.root_password.len() < 8 {
            self.error_message =
                Some("Root password must be at least 8 characters long".to_string());
            return false;
        }
        if self.root_password != self.root_password_confirm {
            self.error_message = Some("Root passwords do not match".to_string());
            return false;
        }
        if self.username.trim().is_empty() {
            self.error_message = Some("Initial username must not be empty".to_string());
            return false;
        }
        if self.user_password.len() < 8 {
            self.error_message =
                Some("User password must be at least 8 characters long".to_string());
            return false;
        }
        if self.user_password != self.user_password_confirm {
            self.error_message = Some("User passwords do not match".to_string());
            return false;
        }
        self.error_message = None;
        true
    }

    /// Renders text lines representing the user account configuration dialog.
    ///
    /// # Returns
    ///
    /// Formatted ASCII line strings.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(
            "┌──────────────────────────────────────────────────────────────────────────┐"
                .to_string(),
        );
        lines.push(
            "│ System Accounts & Administrator Credentials                              │"
                .to_string(),
        );
        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        let root_cursor = if self.focus == UserAccountFocusField::RootPassword {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {root_cursor} Root Password:         {:<49} │",
            "*".repeat(self.root_password.len())
        ));
        let confirm_root_cursor = if self.focus == UserAccountFocusField::RootPasswordConfirm {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {confirm_root_cursor} Confirm Root Pass:     {:<49} │",
            "*".repeat(self.root_password_confirm.len())
        ));
        let user_cursor = if self.focus == UserAccountFocusField::Username {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {user_cursor} Initial Username:       {:<49} │",
            self.username
        ));
        let user_pwd_cursor = if self.focus == UserAccountFocusField::UserPassword {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {user_pwd_cursor} Initial User Password:  {:<49} │",
            "*".repeat(self.user_password.len())
        ));
        let confirm_user_cursor = if self.focus == UserAccountFocusField::UserPasswordConfirm {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "│ {confirm_user_cursor} Confirm User Pass:      {:<49} │",
            "*".repeat(self.user_password_confirm.len())
        ));
        let sudo_cursor = if self.focus == UserAccountFocusField::GrantSudo {
            ">"
        } else {
            " "
        };
        let sudo_box = if self.grant_sudo { "[X]" } else { "[ ]" };
        lines.push(format!(
            "│ {sudo_cursor} Grant Sudo Privileges:  {sudo_box:<49} │"
        ));
        lines.push(
            "├──────────────────────────────────────────────────────────────────────────┤"
                .to_string(),
        );
        if let Some(ref err) = self.error_message {
            lines.push(format!("│ [!] ERROR: {err:<61} │"));
        } else {
            lines.push(
                "│ Passwords must be at least 8 characters long.                            │"
                    .to_string(),
            );
        }
        lines.push(
            "│ [Tab] Next Field  [Space] Toggle Sudo  [Enter] Next  [Esc] Back          │"
                .to_string(),
        );
        lines.push(
            "└──────────────────────────────────────────────────────────────────────────┘"
                .to_string(),
        );
        lines
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
    /// Network configuration dialog state.
    pub network_config: NetworkConfigDialog,
    /// User account and credentials dialog state.
    pub user_account: UserAccountDialog,
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
            network_config: NetworkConfigDialog::default(),
            user_account: UserAccountDialog::default(),
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
                TuiKey::Enter => self.step = WizardStep::NetworkConfig,
                TuiKey::Escape => self.step = WizardStep::PartitionConfirmation,
                _ => {}
            },
            WizardStep::NetworkConfig => match key {
                TuiKey::Tab => self.network_config.toggle_focus(),
                TuiKey::Down => match self.network_config.focus {
                    NetworkFocusField::Interface => self.network_config.select_next_interface(),
                    _ => self.network_config.toggle_focus(),
                },
                TuiKey::Up => match self.network_config.focus {
                    NetworkFocusField::Interface => {
                        if !self.network_config.interfaces.is_empty() {
                            self.network_config.selected_interface =
                                if self.network_config.selected_interface == 0 {
                                    self.network_config.interfaces.len() - 1
                                } else {
                                    self.network_config.selected_interface - 1
                                };
                        }
                    }
                    _ => self.network_config.focus_prev(),
                },
                TuiKey::Space => match self.network_config.focus {
                    NetworkFocusField::Mode => self.network_config.toggle_mode(),
                    NetworkFocusField::Interface => self.network_config.select_next_interface(),
                    _ => {}
                },
                TuiKey::Char(c) => self.network_config.handle_char(c),
                TuiKey::Backspace => self.network_config.handle_backspace(),
                TuiKey::Enter => {
                    if self.network_config.validate() {
                        self.step = WizardStep::UserAccount;
                    }
                }
                TuiKey::Escape => self.step = WizardStep::LocaleKeyboard,
                _ => {}
            },
            WizardStep::UserAccount => match key {
                TuiKey::Tab | TuiKey::Down => self.user_account.toggle_focus(),
                TuiKey::Up => self.user_account.focus_prev(),
                TuiKey::Space => {
                    if self.user_account.focus == UserAccountFocusField::GrantSudo {
                        self.user_account.toggle_sudo();
                    }
                }
                TuiKey::Char(c) => self.user_account.handle_char(c),
                TuiKey::Backspace => self.user_account.handle_backspace(),
                TuiKey::Enter => {
                    if self.user_account.validate() {
                        self.step = WizardStep::Progress;
                    }
                }
                TuiKey::Escape => self.step = WizardStep::NetworkConfig,
                _ => {}
            },
            WizardStep::Progress => {
                if self.progress.overall_percent >= 100 && key == TuiKey::Enter {
                    self.step = WizardStep::Complete;
                }
            }
            WizardStep::Complete => {}
        }
    }

    /// Renders text lines representing the currently active wizard dialog.
    ///
    /// # Returns
    ///
    /// Formatted ASCII line strings.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        match self.step {
            WizardStep::DiskSelection => self.disk_selection.render(),
            WizardStep::PartitionConfirmation => self.partition_confirm.render(),
            WizardStep::LocaleKeyboard => self.locale_keyboard.render(),
            WizardStep::NetworkConfig => self.network_config.render(),
            WizardStep::UserAccount => self.user_account.render(),
            WizardStep::Progress => self.progress.render(),
            WizardStep::Complete => self.complete.render(),
        }
    }

    /// Converts wizard identity settings to a [`crate::platform::linux_config::LinuxIdentityConfig`].
    ///
    /// # Returns
    ///
    /// Configured identity configuration.
    #[must_use]
    pub fn to_linux_identity(&self) -> crate::platform::linux_config::LinuxIdentityConfig {
        crate::platform::linux_config::LinuxIdentityConfig {
            hostname: "localhost".to_string(),
            locale: self.locale_keyboard.current_locale().to_string(),
            keymap: self.locale_keyboard.current_keymap().to_string(),
            os_name: "Linux".to_string(),
            os_version: "1.0".to_string(),
        }
    }

    /// Converts wizard user settings into a [`crate::platform::linux_config::UserProvisioningEngine`].
    ///
    /// # Returns
    ///
    /// Configured user provisioning engine.
    #[must_use]
    pub fn to_user_provisioning(&self) -> crate::platform::linux_config::UserProvisioningEngine {
        let mut engine = crate::platform::linux_config::UserProvisioningEngine::new();
        if !self.user_account.username.is_empty() {
            engine.add_user(crate::platform::linux_config::ProvisionUserAccount {
                username: self.user_account.username.clone(),
                uid: 1000,
                gid: 1000,
                gecos: self.user_account.username.clone(),
                home_dir: format!("/home/{}", self.user_account.username),
                shell: "/bin/bash".to_string(),
                password_hash: self.user_account.user_password.clone(),
            });
        }
        engine
    }

    /// Converts wizard settings to a [`crate::platform::unattend::LinuxCloudInitConfig`].
    ///
    /// # Returns
    ///
    /// Configured cloud-init provisioning configuration.
    #[must_use]
    pub fn to_cloud_init(&self) -> crate::platform::unattend::LinuxCloudInitConfig {
        let mut users = Vec::new();
        if !self.user_account.username.is_empty() {
            users.push(crate::platform::linux_config::ProvisionUserAccount {
                username: self.user_account.username.clone(),
                uid: 1000,
                gid: 1000,
                gecos: self.user_account.username.clone(),
                home_dir: format!("/home/{}", self.user_account.username),
                shell: "/bin/bash".to_string(),
                password_hash: self.user_account.user_password.clone(),
            });
        }
        crate::platform::unattend::LinuxCloudInitConfig {
            hostname: "localhost".to_string(),
            users,
            ssh_authorized_keys: Vec::new(),
            packages: Vec::new(),
            runcmd: Vec::new(),
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

        let lines = dialog.render();
        assert!(lines.iter().any(|l| l.contains("Configure System Locale")));
        assert!(lines.iter().any(|l| l.contains("en_US.UTF-8")));
    }

    /// Tests `NetworkConfigDialog` operations, input handling, validation, and rendering.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_network_config_dialog() {
        let mut dialog = NetworkConfigDialog::new(vec!["eth0".to_string(), "eth1".to_string()]);
        assert_eq!(dialog.focus, NetworkFocusField::Interface);
        assert_eq!(dialog.mode, NetworkConfigMode::Dhcp);
        assert_eq!(dialog.interfaces.len(), 2);

        // Cycle interfaces
        dialog.select_next_interface();
        assert_eq!(dialog.selected_interface, 1);
        dialog.select_next_interface();
        assert_eq!(dialog.selected_interface, 0);

        // Cycle modes
        dialog.toggle_mode();
        assert_eq!(dialog.mode, NetworkConfigMode::Static);
        dialog.toggle_mode();
        assert_eq!(dialog.mode, NetworkConfigMode::Skip);
        dialog.toggle_mode();
        assert_eq!(dialog.mode, NetworkConfigMode::Dhcp);

        // Focus navigation forwards
        dialog.toggle_focus();
        assert_eq!(dialog.focus, NetworkFocusField::Mode);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, NetworkFocusField::IpAddress);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, NetworkFocusField::SubnetMask);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, NetworkFocusField::Gateway);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, NetworkFocusField::DnsServer);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, NetworkFocusField::Interface);

        // Focus navigation backwards
        dialog.focus_prev();
        assert_eq!(dialog.focus, NetworkFocusField::DnsServer);
        dialog.focus_prev();
        assert_eq!(dialog.focus, NetworkFocusField::Gateway);
        dialog.focus_prev();
        assert_eq!(dialog.focus, NetworkFocusField::SubnetMask);
        dialog.focus_prev();
        assert_eq!(dialog.focus, NetworkFocusField::IpAddress);
        dialog.focus_prev();
        assert_eq!(dialog.focus, NetworkFocusField::Mode);
        dialog.focus_prev();
        assert_eq!(dialog.focus, NetworkFocusField::Interface);

        // Empty interfaces select_next_interface
        let mut empty_iface_dialog = NetworkConfigDialog::new(Vec::new());
        empty_iface_dialog.select_next_interface();
        assert_eq!(empty_iface_dialog.selected_interface, 0);

        // render_text on all focus fields
        for f in [
            NetworkFocusField::Interface,
            NetworkFocusField::Mode,
            NetworkFocusField::IpAddress,
            NetworkFocusField::SubnetMask,
            NetworkFocusField::Gateway,
            NetworkFocusField::DnsServer,
        ] {
            dialog.focus = f;
            assert_ne!(dialog.render(), Vec::<String>::new());
        }

        // Input handling on Gateway
        dialog.focus = NetworkFocusField::Gateway;
        dialog.gateway.clear();
        dialog.handle_char('1');
        dialog.handle_char('0');
        dialog.handle_char('.');
        dialog.handle_char('\n'); // Control char ignored
        assert_eq!(dialog.gateway, "10.");
        dialog.handle_backspace();
        assert_eq!(dialog.gateway, "10");

        // Input on IP Address
        dialog.focus = NetworkFocusField::IpAddress;
        dialog.ip_address.clear();
        dialog.handle_char('1');
        dialog.handle_char('9');
        dialog.handle_char('2');
        assert_eq!(dialog.ip_address, "192");
        dialog.handle_backspace();
        assert_eq!(dialog.ip_address, "19");

        // Input on SubnetMask
        dialog.focus = NetworkFocusField::SubnetMask;
        dialog.subnet_mask.clear();
        dialog.handle_char('2');
        assert_eq!(dialog.subnet_mask, "2");
        dialog.handle_backspace();
        assert_eq!(dialog.subnet_mask, "");

        // Input on DnsServer
        dialog.focus = NetworkFocusField::DnsServer;
        dialog.dns_servers.clear();
        dialog.handle_char('1');
        assert_eq!(dialog.dns_servers, vec!["1".to_string()]);
        dialog.handle_char('0');
        assert_eq!(dialog.dns_servers, vec!["10".to_string()]);
        dialog.handle_backspace();
        assert_eq!(dialog.dns_servers, vec!["1".to_string()]);
        dialog.dns_servers.clear();
        dialog.handle_backspace();
        assert_eq!(dialog.dns_servers, Vec::<String>::new());

        // Ignored input on non-text fields
        dialog.focus = NetworkFocusField::Mode;
        dialog.handle_char('x');
        dialog.handle_backspace();

        // Validation: DHCP passes automatically
        dialog.mode = NetworkConfigMode::Dhcp;
        assert!(dialog.validate());
        assert!(dialog.error_message.is_none());

        // Validation: Static with invalid IP fails
        dialog.mode = NetworkConfigMode::Static;
        dialog.ip_address = "invalid_ip".to_string();
        dialog.subnet_mask = "255.255.255.0".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid static IP"));

        // Validation: Static with invalid subnet mask fails
        dialog.ip_address = "192.168.1.50".to_string();
        dialog.subnet_mask = "not_a_mask".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid subnet mask"));

        // Validation: Static with invalid gateway fails
        dialog.subnet_mask = "255.255.255.0".to_string();
        dialog.gateway = "bad_gateway".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid gateway"));

        // Validation: Valid static configuration passes
        dialog.gateway = "192.168.1.1".to_string();
        assert!(dialog.validate());
        assert!(dialog.error_message.is_none());

        // Validation: Static configuration with empty gateway is allowed
        dialog.gateway.clear();
        assert!(dialog.validate());
        assert!(dialog.error_message.is_none());

        // Rendering with error and without error
        let normal_render = dialog.render();
        assert!(normal_render
            .iter()
            .any(|l| l.contains("Network Interface & Address Assignment")));
        assert!(normal_render.iter().any(|l| l.contains("192.168.1.50")));

        dialog.error_message = Some("Sample error".to_string());
        let err_render = dialog.render();
        assert!(err_render.iter().any(|l| l.contains("ERROR: Sample error")));
    }

    /// Tests `UserAccountDialog` operations, input handling, validation, and rendering.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_user_account_dialog() {
        let mut dialog = UserAccountDialog::new();
        assert_eq!(dialog.focus, UserAccountFocusField::RootPassword);
        assert!(dialog.grant_sudo);

        // Toggle sudo
        dialog.toggle_sudo();
        assert!(!dialog.grant_sudo);
        dialog.toggle_sudo();
        assert!(dialog.grant_sudo);

        // Focus forward and backward
        dialog.toggle_focus();
        assert_eq!(dialog.focus, UserAccountFocusField::RootPasswordConfirm);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, UserAccountFocusField::Username);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, UserAccountFocusField::UserPassword);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, UserAccountFocusField::UserPasswordConfirm);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, UserAccountFocusField::GrantSudo);
        dialog.toggle_focus();
        assert_eq!(dialog.focus, UserAccountFocusField::RootPassword);

        dialog.focus_prev();
        assert_eq!(dialog.focus, UserAccountFocusField::GrantSudo);
        dialog.focus_prev();
        assert_eq!(dialog.focus, UserAccountFocusField::UserPasswordConfirm);
        dialog.focus_prev();
        assert_eq!(dialog.focus, UserAccountFocusField::UserPassword);
        dialog.focus_prev();
        assert_eq!(dialog.focus, UserAccountFocusField::Username);
        dialog.focus_prev();
        assert_eq!(dialog.focus, UserAccountFocusField::RootPasswordConfirm);
        dialog.focus_prev();
        assert_eq!(dialog.focus, UserAccountFocusField::RootPassword);

        // Render on all focus fields
        for f in [
            UserAccountFocusField::RootPassword,
            UserAccountFocusField::RootPasswordConfirm,
            UserAccountFocusField::Username,
            UserAccountFocusField::UserPassword,
            UserAccountFocusField::UserPasswordConfirm,
            UserAccountFocusField::GrantSudo,
        ] {
            dialog.focus = f;
            assert_ne!(dialog.render(), Vec::<String>::new());
        }

        // Input handling on UserPasswordConfirm
        dialog.focus = UserAccountFocusField::UserPasswordConfirm;
        dialog.user_password_confirm.clear();
        dialog.handle_char('a');
        dialog.handle_char('b');
        dialog.handle_char('\t'); // Control char ignored
        assert_eq!(dialog.user_password_confirm, "ab");
        dialog.handle_backspace();
        assert_eq!(dialog.user_password_confirm, "a");

        // Input on RootPassword
        dialog.focus = UserAccountFocusField::RootPassword;
        dialog.root_password.clear();
        dialog.handle_char('p');
        dialog.handle_char('a');
        assert_eq!(dialog.root_password, "pa");
        dialog.handle_backspace();
        assert_eq!(dialog.root_password, "p");

        // Input on RootPasswordConfirm
        dialog.focus = UserAccountFocusField::RootPasswordConfirm;
        dialog.root_password_confirm.clear();
        dialog.handle_char('p');
        assert_eq!(dialog.root_password_confirm, "p");
        dialog.handle_backspace();
        assert_eq!(dialog.root_password_confirm, "");

        // Input on Username
        dialog.focus = UserAccountFocusField::Username;
        dialog.username.clear();
        dialog.handle_char('u');
        assert_eq!(dialog.username, "u");
        dialog.handle_backspace();
        assert_eq!(dialog.username, "");

        // Input on UserPassword
        dialog.focus = UserAccountFocusField::UserPassword;
        dialog.user_password.clear();
        dialog.handle_char('x');
        assert_eq!(dialog.user_password, "x");
        dialog.handle_backspace();
        assert_eq!(dialog.user_password, "");

        // Input on GrantSudo is ignored
        dialog.focus = UserAccountFocusField::GrantSudo;
        dialog.handle_char('x');
        dialog.handle_backspace();

        // Validation: Short root password
        dialog.root_password = "short".to_string();
        dialog.root_password_confirm = "short".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Root password must be at least 8"));

        // Validation: Root password mismatch
        dialog.root_password = "password123".to_string();
        dialog.root_password_confirm = "password456".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Root passwords do not match"));

        // Validation: Empty username
        dialog.root_password_confirm = "password123".to_string();
        dialog.username = "   ".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("username must not be empty"));

        // Validation: Short user password
        dialog.username = "developer".to_string();
        dialog.user_password = "pw".to_string();
        dialog.user_password_confirm = "pw".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("User password must be at least 8"));

        // Validation: User password mismatch
        dialog.user_password = "userpassword1".to_string();
        dialog.user_password_confirm = "userpassword2".to_string();
        assert!(!dialog.validate());
        assert!(dialog
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("User passwords do not match"));

        // Validation: All valid
        dialog.user_password_confirm = "userpassword1".to_string();
        assert!(dialog.validate());
        assert!(dialog.error_message.is_none());

        // Render check
        let lines = dialog.render();
        assert!(lines
            .iter()
            .any(|l| l.contains("System Accounts & Administrator Credentials")));
        assert!(lines.iter().any(|l| l.contains("developer")));
        assert!(lines.iter().any(|l| l.contains("[X]")));

        dialog.error_message = Some("User account error test".to_string());
        let err_lines = dialog.render();
        assert!(err_lines
            .iter()
            .any(|l| l.contains("ERROR: User account error test")));
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
    #[allow(clippy::too_many_lines)]
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

        // Advance to NetworkConfig
        assert_eq!(wizard.step, WizardStep::LocaleKeyboard);
        assert_ne!(wizard.render(), Vec::<String>::new());
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::NetworkConfig);
        assert_ne!(wizard.render(), Vec::<String>::new());

        // Escape in NetworkConfig retreats to LocaleKeyboard
        wizard.handle_key(TuiKey::Escape);
        assert_eq!(wizard.step, WizardStep::LocaleKeyboard);
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::NetworkConfig);

        // NetworkConfig key handling
        wizard.network_config.focus = NetworkFocusField::Interface;
        wizard.handle_key(TuiKey::Down);
        wizard.handle_key(TuiKey::Space);
        wizard.handle_key(TuiKey::Up);
        wizard.network_config.selected_interface = 1;
        wizard.handle_key(TuiKey::Up);
        let ifaces_saved = std::mem::take(&mut wizard.network_config.interfaces);
        wizard.handle_key(TuiKey::Up);
        wizard.network_config.interfaces = ifaces_saved;
        wizard.network_config.focus = NetworkFocusField::Mode;
        wizard.handle_key(TuiKey::Down);
        wizard.handle_key(TuiKey::Up);
        wizard.network_config.focus = NetworkFocusField::Mode;
        wizard.handle_key(TuiKey::Space); // Toggles mode
        wizard.network_config.focus = NetworkFocusField::IpAddress;
        wizard.handle_key(TuiKey::Space); // Space on IpAddress does nothing
        wizard.handle_key(TuiKey::F(5)); // Unhandled key on NetworkConfig
        wizard.handle_key(TuiKey::Tab);
        wizard.handle_key(TuiKey::Char('a'));
        wizard.handle_key(TuiKey::Backspace);

        // Invalid network config prevents advancing
        wizard.network_config.mode = NetworkConfigMode::Static;
        wizard.network_config.ip_address = "invalid_static_ip".to_string();
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::NetworkConfig);

        // Set valid network config and advance to UserAccount
        wizard.network_config.mode = NetworkConfigMode::Dhcp;
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::UserAccount);
        assert_ne!(wizard.render(), Vec::<String>::new());

        // Escape in UserAccount retreats to NetworkConfig
        wizard.handle_key(TuiKey::Escape);
        assert_eq!(wizard.step, WizardStep::NetworkConfig);
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::UserAccount);

        // UserAccount key handling
        wizard.handle_key(TuiKey::Tab);
        wizard.handle_key(TuiKey::Down);
        wizard.handle_key(TuiKey::Up);
        wizard.handle_key(TuiKey::F(5)); // Unhandled key on UserAccount
        wizard.user_account.focus = UserAccountFocusField::GrantSudo;
        wizard.handle_key(TuiKey::Space); // Toggles sudo
        wizard.user_account.focus = UserAccountFocusField::RootPassword;
        wizard.handle_key(TuiKey::Space); // Space on non-sudo does nothing
        wizard.handle_key(TuiKey::Up); // focus_prev
        wizard.handle_key(TuiKey::Char('1'));
        wizard.handle_key(TuiKey::Backspace);

        // Invalid credentials prevents advancing
        wizard.user_account.root_password = "bad".to_string();
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::UserAccount);

        // Valid credentials advances to Progress
        wizard.user_account.root_password = "rootpassword123".to_string();
        wizard.user_account.root_password_confirm = "rootpassword123".to_string();
        wizard.user_account.username = "sysadmin".to_string();
        wizard.user_account.user_password = "userpassword123".to_string();
        wizard.user_account.user_password_confirm = "userpassword123".to_string();
        wizard.user_account.grant_sudo = true;
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::Progress);
        assert_ne!(wizard.render(), Vec::<String>::new());

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
        assert_ne!(wizard.render(), Vec::<String>::new());

        // Keys on Complete step
        wizard.handle_key(TuiKey::Enter);
        assert_eq!(wizard.step, WizardStep::Complete);

        let comp_lines = wizard.complete.render();
        assert!(comp_lines
            .iter()
            .any(|l| l.contains("Installation Complete!")));

        // Test deployment engine conversions
        let identity = wizard.to_linux_identity();
        assert_eq!(identity.hostname, "localhost");
        assert_eq!(identity.locale, "en_US.UTF-8");
        assert_eq!(identity.keymap, "us");
        assert_eq!(identity.os_name, "Linux");

        let user_prov = wizard.to_user_provisioning();
        assert!(user_prov.render_passwd().contains("sysadmin"));

        let cloud_init = wizard.to_cloud_init();
        assert_eq!(cloud_init.hostname, "localhost");
        assert_eq!(cloud_init.users.len(), 1);
        assert_eq!(cloud_init.users[0].username, "sysadmin");

        // Empty user account produces empty user lists
        let mut empty_user_wizard = wizard.clone();
        empty_user_wizard.user_account.username.clear();
        assert_eq!(
            empty_user_wizard.to_cloud_init().users,
            Vec::<crate::platform::linux_config::ProvisionUserAccount>::new()
        );
        assert_eq!(empty_user_wizard.to_user_provisioning().render_passwd(), "");

        // Test wizard empty target disks fallback
        let mut empty_wizard = BareMetalInstallationWizard::new(Vec::new());
        assert_eq!(
            empty_wizard.partition_confirm.target_path,
            BlockDevicePath::new("/dev/sda")
        );
        assert_eq!(empty_wizard.partition_confirm.disk_size_gib, 64);
        assert_ne!(empty_wizard.render(), Vec::<String>::new());
        empty_wizard.handle_key(TuiKey::Enter);
        assert_eq!(empty_wizard.step, WizardStep::PartitionConfirmation);
        assert_ne!(empty_wizard.render(), Vec::<String>::new());
    }
}

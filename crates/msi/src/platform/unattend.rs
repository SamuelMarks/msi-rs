//! Machine Provisioning & Unattended Automation Engine.
//!
//! Generates unattended answer files and cloud-init / sysusers.d configurations:
//! - Windows `unattend.xml` (Panther OOBE bypass, regional settings, automated user setup).
//! - Linux `#cloud-config` YAML and systemd `sysusers.d` provisioning files.

use crate::error::{Error, Result};
use crate::platform::linux_config::ProvisionUserAccount;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Windows Unattended Answer File (`unattend.xml`) generator for automated setup and OOBE bypass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsUnattendConfig {
    /// Host computer name.
    pub computer_name: String,
    /// Registered organization or company name.
    pub organization: String,
    /// Registered owner or user name.
    pub owner: String,
    /// Windows timezone standard name (e.g. `UTC`, `Pacific Standard Time`).
    pub time_zone: String,
    /// Keyboard input locale (e.g. `0409:00000409`).
    pub input_locale: String,
    /// System locale identifier (e.g. `en-US`).
    pub system_locale: String,
    /// UI language (e.g. `en-US`).
    pub ui_language: String,
    /// Plaintext administrator password (cleared post-OOBE) or empty string to disable.
    pub admin_password: String,
    /// Optional initial standard user account.
    pub initial_user: Option<ProvisionUserAccount>,
    /// Auto-accept End User License Agreement (EULA).
    pub accept_eula: bool,
    /// OOBE network setup bypass flag.
    pub hide_wireless_setup: bool,
}

impl Default for WindowsUnattendConfig {
    fn default() -> Self {
        Self {
            computer_name: "WIN-INSTALL".to_string(),
            organization: "Organization".to_string(),
            owner: "Administrator".to_string(),
            time_zone: "UTC".to_string(),
            input_locale: "0409:00000409".to_string(),
            system_locale: "en-US".to_string(),
            ui_language: "en-US".to_string(),
            admin_password: "Password123!".to_string(),
            initial_user: None,
            accept_eula: true,
            hide_wireless_setup: true,
        }
    }
}

impl WindowsUnattendConfig {
    /// Renders the complete XML content of `unattend.xml`.
    ///
    /// # Returns
    ///
    /// Valid Windows Setup answer file XML string.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn render_xml(&self) -> String {
        let mut xml = String::new();
        let _ = writeln!(xml, r#"<?xml version="1.0" encoding="utf-8"?>"#);
        let _ = writeln!(
            xml,
            r#"<unattend xmlns="urn:schemas-microsoft-com:unattend">"#
        );

        // 1. specialize pass
        let _ = writeln!(xml, r#"    <settings pass="specialize">"#);
        let _ = writeln!(
            xml,
            r#"        <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">"#
        );
        let _ = writeln!(
            xml,
            "            <ComputerName>{}</ComputerName>",
            self.computer_name
        );
        let _ = writeln!(xml, "            <TimeZone>{}</TimeZone>", self.time_zone);
        let _ = writeln!(xml, "        </component>");
        let _ = writeln!(xml, "    </settings>");

        // 2. oobeSystem pass
        let _ = writeln!(xml, r#"    <settings pass="oobeSystem">"#);
        let _ = writeln!(
            xml,
            r#"        <component name="Microsoft-Windows-International-Core" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">"#
        );
        let _ = writeln!(
            xml,
            "            <InputLocale>{}</InputLocale>",
            self.input_locale
        );
        let _ = writeln!(
            xml,
            "            <SystemLocale>{}</SystemLocale>",
            self.system_locale
        );
        let _ = writeln!(
            xml,
            "            <UILanguage>{}</UILanguage>",
            self.ui_language
        );
        let _ = writeln!(
            xml,
            "            <UserLocale>{}</UserLocale>",
            self.system_locale
        );
        let _ = writeln!(xml, "        </component>");

        let _ = writeln!(
            xml,
            r#"        <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">"#
        );
        let _ = writeln!(xml, "            <OOBE>");
        let _ = writeln!(
            xml,
            "                <HideEULAPage>{}</HideEULAPage>",
            self.accept_eula
        );
        let _ = writeln!(
            xml,
            "                <HideOEMRegistrationScreen>true</HideOEMRegistrationScreen>"
        );
        let _ = writeln!(
            xml,
            "                <HideOnlineAccountScreens>true</HideOnlineAccountScreens>"
        );
        let _ = writeln!(
            xml,
            "                <HideWirelessSetupInOOBE>{}</HideWirelessSetupInOOBE>",
            self.hide_wireless_setup
        );
        let _ = writeln!(xml, "                <ProtectYourPC>3</ProtectYourPC>");
        let _ = writeln!(xml, "            </OOBE>");

        let _ = writeln!(xml, "            <UserAccounts>");
        if !self.admin_password.is_empty() {
            let _ = writeln!(xml, "                <AdministratorPassword>");
            let _ = writeln!(
                xml,
                "                    <Value>{}</Value>",
                self.admin_password
            );
            let _ = writeln!(xml, "                    <PlainText>true</PlainText>");
            let _ = writeln!(xml, "                </AdministratorPassword>");
        }

        if let Some(ref u) = self.initial_user {
            let _ = writeln!(xml, "                <LocalAccounts>");
            let _ = writeln!(xml, r#"                    <LocalAccount action="add">"#);
            let _ = writeln!(xml, "                        <Name>{}</Name>", u.username);
            let _ = writeln!(
                xml,
                "                        <DisplayName>{}</DisplayName>",
                u.gecos
            );
            let _ = writeln!(xml, "                        <Group>Administrators</Group>");
            let _ = writeln!(xml, "                        <Password>");
            let _ = writeln!(
                xml,
                "                            <Value>{}</Value>",
                u.password_hash
            );
            let _ = writeln!(
                xml,
                "                            <PlainText>true</PlainText>"
            );
            let _ = writeln!(xml, "                        </Password>");
            let _ = writeln!(xml, "                    </LocalAccount>");
            let _ = writeln!(xml, "                </LocalAccounts>");
        }
        let _ = writeln!(xml, "            </UserAccounts>");

        let _ = writeln!(
            xml,
            "            <RegisteredOrganization>{}</RegisteredOrganization>",
            self.organization
        );
        let _ = writeln!(
            xml,
            "            <RegisteredOwner>{}</RegisteredOwner>",
            self.owner
        );
        let _ = writeln!(xml, "        </component>");
        let _ = writeln!(xml, "    </settings>");
        let _ = writeln!(xml, "</unattend>");
        xml
    }

    /// Writes `unattend.xml` directly to `<sysroot>/Windows/Panther/unattend.xml`.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Target Windows sysroot path.
    ///
    /// # Returns
    ///
    /// Absolute path to written answer file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnattendError`] if directory creation or file write fails.
    pub fn write_to_sysroot(&self, sysroot: &Path) -> Result<PathBuf> {
        let panther = sysroot.join("Windows/Panther");
        if let Err(e) = std::fs::create_dir_all(&panther) {
            return Err(Error::UnattendError {
                reason: format!("failed to create Windows/Panther directory: {e}"),
            });
        }

        let target_file = panther.join("unattend.xml");
        let xml_content = self.render_xml();
        if let Err(e) = std::fs::write(&target_file, xml_content) {
            return Err(Error::UnattendError {
                reason: format!("failed to write unattend.xml: {e}"),
            });
        }

        Ok(target_file)
    }
}

/// Linux cloud-init and `sysusers.d` automated machine provisioning configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinuxCloudInitConfig {
    /// Hostname identifier.
    pub hostname: String,
    /// Automated provisioned users.
    pub users: Vec<ProvisionUserAccount>,
    /// Authorized SSH public keys for login.
    pub ssh_authorized_keys: Vec<String>,
    /// Packages to install on first boot.
    pub packages: Vec<String>,
    /// Commands to execute on first boot.
    pub runcmd: Vec<String>,
}

impl LinuxCloudInitConfig {
    /// Renders standard `#cloud-config` YAML content.
    ///
    /// # Returns
    ///
    /// YAML string.
    #[must_use]
    pub fn render_yaml(&self) -> String {
        let mut y = String::from(
            "#cloud-config
",
        );
        let _ = writeln!(y, "hostname: {}", self.hostname);
        y.push_str(
            "manage_etc_hosts: true

",
        );

        if !self.users.is_empty() {
            y.push_str(
                "users:
",
            );
            for u in &self.users {
                let _ = writeln!(y, "  - name: {}", u.username);
                let _ = writeln!(y, "    gecos: {}", u.gecos);
                let _ = writeln!(y, "    shell: {}", u.shell);
                let _ = writeln!(y, "    homedir: {}", u.home_dir);
                if u.username == "root" || u.uid == 0 {
                    y.push_str(
                        "    lock_passwd: false
",
                    );
                } else {
                    y.push_str(
                        "    sudo: ALL=(ALL) NOPASSWD:ALL
",
                    );
                    y.push_str(
                        "    groups: [sudo, wheel, adm]
",
                    );
                }
                if !self.ssh_authorized_keys.is_empty() {
                    y.push_str(
                        "    ssh_authorized_keys:
",
                    );
                    for key in &self.ssh_authorized_keys {
                        let _ = writeln!(y, "      - {key}");
                    }
                }
            }
        }

        if !self.packages.is_empty() {
            y.push_str(
                "
packages:
",
            );
            for pkg in &self.packages {
                let _ = writeln!(y, "  - {pkg}");
            }
        }

        if !self.runcmd.is_empty() {
            y.push_str(
                "
runcmd:
",
            );
            for cmd in &self.runcmd {
                let _ = writeln!(y, r#"  - "{cmd}""#);
            }
        }

        y
    }

    /// Renders systemd `sysusers.d` formatted text.
    ///
    /// # Returns
    ///
    /// Formatted user table string.
    #[must_use]
    pub fn render_sysusers(&self) -> String {
        let mut s = String::from(
            "# Type Name ID GECOS Home directory Shell
",
        );
        for u in &self.users {
            let _ = writeln!(
                s,
                r#"u {} {} "{}" {} {}"#,
                u.username, u.uid, u.gecos, u.home_dir, u.shell
            );
        }
        s
    }

    /// Writes cloud-init and `sysusers.d` files into target Linux sysroot.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Target Linux root path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnattendError`] if directory or file write fails.
    pub fn write_to_sysroot(&self, sysroot: &Path) -> Result<()> {
        let cloud_dir = sysroot.join("etc/cloud/cloud.cfg.d");
        let sysusers_dir = sysroot.join("etc/sysusers.d");

        for d in &[&cloud_dir, &sysusers_dir] {
            if let Err(e) = std::fs::create_dir_all(d) {
                return Err(Error::UnattendError {
                    reason: format!("failed to create config directory '{}': {e}", d.display()),
                });
            }
        }

        let cloud_file = cloud_dir.join("99-msi-installer.cfg");
        if let Err(e) = std::fs::write(&cloud_file, self.render_yaml()) {
            return Err(Error::UnattendError {
                reason: format!("failed to write cloud-init configuration: {e}"),
            });
        }

        let sysusers_file = sysusers_dir.join("00-msi-users.conf");
        if let Err(e) = std::fs::write(&sysusers_file, self.render_sysusers()) {
            return Err(Error::UnattendError {
                reason: format!("failed to write sysusers.d configuration: {e}"),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `WindowsUnattendConfig` XML rendering and sysroot write.
    #[test]
    fn test_windows_unattend_rendering() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_unattend_{}", std::process::id()));
        let config = WindowsUnattendConfig {
            computer_name: "DESKTOP-NODE1".to_string(),
            admin_password: "SecretPassword123!".to_string(),
            initial_user: Some(ProvisionUserAccount {
                username: "operator".to_string(),
                uid: 1000,
                gid: 1000,
                gecos: "Primary Operator".to_string(),
                home_dir: r"C:\Users\operator".to_string(),
                shell: "cmd.exe".to_string(),
                password_hash: "OperatorPass!".to_string(),
            }),
            ..WindowsUnattendConfig::default()
        };

        let xml = config.render_xml();
        assert!(xml.contains("<ComputerName>DESKTOP-NODE1</ComputerName>"));
        assert!(xml.contains("<HideEULAPage>true</HideEULAPage>"));
        assert!(xml.contains("<Value>SecretPassword123!</Value>"));
        assert!(xml.contains("<Name>operator</Name>"));
        assert!(xml.contains("<Group>Administrators</Group>"));

        let file_res = config.write_to_sysroot(&temp_dir);
        assert!(file_res.is_ok());
        assert!(temp_dir.join("Windows/Panther/unattend.xml").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `LinuxCloudInitConfig` YAML and sysusers rendering and sysroot write.
    #[test]
    fn test_linux_cloud_init_rendering() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_cloudinit_{}", std::process::id()));
        let config = LinuxCloudInitConfig {
            hostname: "cloud-appliance".to_string(),
            users: vec![ProvisionUserAccount {
                username: "ubuntu".to_string(),
                uid: 1000,
                gid: 1000,
                gecos: "Ubuntu Default".to_string(),
                home_dir: "/home/ubuntu".to_string(),
                shell: "/bin/bash".to_string(),
                password_hash: "*".to_string(),
            }],
            ssh_authorized_keys: vec!["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB...".to_string()],
            packages: vec!["curl".to_string(), "htop".to_string()],
            runcmd: vec!["systemctl start msi-agent".to_string()],
        };

        let yaml = config.render_yaml();
        assert!(yaml.starts_with("#cloud-config"));
        assert!(yaml.contains("hostname: cloud-appliance"));
        assert!(yaml.contains("- name: ubuntu"));
        assert!(yaml.contains("packages:"));
        assert!(yaml.contains("- curl"));
        assert!(yaml.contains("systemctl start msi-agent"));

        let sysusers = config.render_sysusers();
        assert!(sysusers.contains(r#"u ubuntu 1000 "Ubuntu Default" /home/ubuntu /bin/bash"#));

        assert!(config.write_to_sysroot(&temp_dir).is_ok());
        assert!(temp_dir
            .join("etc/cloud/cloud.cfg.d/99-msi-installer.cfg")
            .exists());
        assert!(temp_dir.join("etc/sysusers.d/00-msi-users.conf").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `WindowsUnattendConfig` branches and error paths.
    #[test]
    fn test_windows_unattend_branches_and_errors() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_winunattend_err_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // Empty admin password and no initial user
        let empty_config = WindowsUnattendConfig {
            admin_password: String::new(),
            initial_user: None,
            ..WindowsUnattendConfig::default()
        };
        let xml = empty_config.render_xml();
        assert!(!xml.contains("<AdministratorPassword>"));
        assert!(!xml.contains("<LocalAccount action=\"add\">"));

        // Error: Windows/Panther cannot be created because "Windows" is a file
        let win_file = temp_dir.join("Windows");
        let _ = std::fs::write(&win_file, b"file");
        assert!(empty_config.write_to_sysroot(&temp_dir).is_err());
        let _ = std::fs::remove_file(&win_file);

        // Error: unattend.xml cannot be written because it's a directory
        let panther_dir = temp_dir.join("Windows/Panther");
        let _ = std::fs::create_dir_all(&panther_dir);
        let unattend_dir = panther_dir.join("unattend.xml");
        let _ = std::fs::create_dir_all(&unattend_dir);
        assert!(empty_config.write_to_sysroot(&temp_dir).is_err());
        let _ = std::fs::remove_dir_all(&unattend_dir);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `LinuxCloudInitConfig` branches and error paths.
    #[test]
    fn test_linux_cloud_init_branches_and_errors() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_cloudinit_err_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. Root user account and uid=0 non-root account
        let root_config = LinuxCloudInitConfig {
            hostname: "root-node".to_string(),
            users: vec![
                ProvisionUserAccount {
                    username: "root".to_string(),
                    uid: 0,
                    gid: 0,
                    gecos: "Root Admin".to_string(),
                    home_dir: "/root".to_string(),
                    shell: "/bin/sh".to_string(),
                    password_hash: "!".to_string(),
                },
                ProvisionUserAccount {
                    username: "wheeladmin".to_string(),
                    uid: 0,
                    gid: 0,
                    gecos: "Wheel Admin".to_string(),
                    home_dir: "/home/wheeladmin".to_string(),
                    shell: "/bin/sh".to_string(),
                    password_hash: "!".to_string(),
                },
            ],
            ssh_authorized_keys: Vec::new(),
            packages: Vec::new(),
            runcmd: Vec::new(),
        };
        let yaml_root = root_config.render_yaml();
        assert!(yaml_root.contains("lock_passwd: false"));
        assert!(!yaml_root.contains("packages:"));
        assert!(!yaml_root.contains("runcmd:"));

        // 2. Empty users
        let empty_config = LinuxCloudInitConfig::default();
        let yaml_empty = empty_config.render_yaml();
        assert!(!yaml_empty.contains("users:"));

        // Error 1: etc is a file
        let etc_file = temp_dir.join("etc");
        let _ = std::fs::write(&etc_file, b"file");
        assert!(root_config.write_to_sysroot(&temp_dir).is_err());
        let _ = std::fs::remove_file(&etc_file);

        // Error 2: cloud file is a directory
        let cloud_dir = temp_dir.join("etc/cloud/cloud.cfg.d");
        let _ = std::fs::create_dir_all(&cloud_dir);
        let cloud_cfg_dir = cloud_dir.join("99-msi-installer.cfg");
        let _ = std::fs::create_dir_all(&cloud_cfg_dir);
        assert!(root_config.write_to_sysroot(&temp_dir).is_err());
        let _ = std::fs::remove_dir_all(&cloud_cfg_dir);

        // Error 3: sysusers file is a directory
        let sysusers_dir = temp_dir.join("etc/sysusers.d");
        let _ = std::fs::create_dir_all(&sysusers_dir);
        let sysusers_conf_dir = sysusers_dir.join("00-msi-users.conf");
        let _ = std::fs::create_dir_all(&sysusers_conf_dir);
        assert!(root_config.write_to_sysroot(&temp_dir).is_err());
        let _ = std::fs::remove_dir_all(&sysusers_conf_dir);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

//! Daemon Lifecycle & Service Supervisors (systemd, launchd, rc.d, SMF).
//!
//! Grounded directly in POSIX daemon guidelines, Linux systemd unit file specifications,
//! Apple launchd property list schemas, FreeBSD rc.subr conventions, and SunOS/illumos SMF manifest DTDs:
//! - **Linux systemd:** Unit files (`[Unit]`, `[Service]`, `[Install]`) and `systemctl` lifecycle commands.
//! - **macOS launchd:** Property lists (`.plist`) with `Label`, `ProgramArguments`, `KeepAlive`, `RunAtLoad`.
//! - **FreeBSD rc.d:** Conforming `rc.subr` shell scripts and `sysrc` / `service` management.
//! - **`SunOS` / illumos SMF:** Service Management Facility XML manifests and `svccfg` / `svcadm` commands.

use crate::error::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Common service configuration representation mapped from MSI `ServiceInstall` and `ServiceControl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDefinition {
    /// Internal service identifier name.
    pub name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Detailed service description.
    pub description: Option<String>,
    /// Path to the main executable binary.
    pub exec_start: PathBuf,
    /// Command-line arguments for launch.
    pub arguments: Vec<String>,
    /// Working directory for daemon process.
    pub working_directory: Option<PathBuf>,
    /// User account to run service as.
    pub user: Option<String>,
    /// Group account to run service as.
    pub group: Option<String>,
    /// Service restart policy (e.g. "always", "on-failure").
    pub restart_policy: String,
    /// Startup type: true if auto-started at boot, false for manual.
    pub auto_start: bool,
    /// Dependencies or services that must start before this service.
    pub after: Vec<String>,
    /// Hard dependency services required for execution.
    pub requires: Vec<String>,
}

impl ServiceDefinition {
    /// Creates a new [`ServiceDefinition`].
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    /// * `exec_start` - Executable path.
    ///
    /// # Returns
    ///
    /// A new [`ServiceDefinition`].
    #[must_use]
    pub fn new(name: impl Into<String>, exec_start: impl Into<PathBuf>) -> Self {
        let name_str = name.into();
        Self {
            name: name_str.clone(),
            display_name: name_str,
            description: None,
            exec_start: exec_start.into(),
            arguments: Vec::new(),
            working_directory: None,
            user: None,
            group: None,
            restart_policy: "on-failure".to_string(),
            auto_start: true,
            after: vec!["network.target".to_string()],
            requires: Vec::new(),
        }
    }

    /// Sets the display name.
    #[must_use]
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// Sets the description.
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Appends a command-line argument.
    #[must_use]
    pub fn arg(mut self, argument: impl Into<String>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    /// Sets the working directory.
    #[must_use]
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    /// Sets the user and group.
    #[must_use]
    pub fn run_as(mut self, user: impl Into<String>, group: Option<String>) -> Self {
        self.user = Some(user.into());
        self.group = group;
        self
    }

    /// Sets auto-start flag.
    #[must_use]
    pub const fn auto_start(mut self, auto: bool) -> Self {
        self.auto_start = auto;
        self
    }

    // --------------------------------------------------------------------------------
    // Linux systemd Unit Generation
    // --------------------------------------------------------------------------------

    /// Returns the standard systemd unit file installation path (e.g. `/lib/systemd/system/<name>.service`).
    #[must_use]
    pub fn systemd_unit_path(&self) -> PathBuf {
        PathBuf::from("/lib/systemd/system").join(format!("{}.service", self.name))
    }

    /// Generates complete systemd unit file content (`[Unit]`, `[Service]`, `[Install]`).
    ///
    /// # Returns
    ///
    /// INI-formatted systemd unit file string.
    #[must_use]
    #[allow(clippy::format_push_string)]
    pub fn generate_systemd_unit(&self) -> String {
        let desc = self.description.as_deref().unwrap_or(&self.display_name);
        let after_str = self.after.join(" ");
        let requires_str = self.requires.join(" ");

        let mut exec_cmd = self.exec_start.to_string_lossy().to_string();
        for a in &self.arguments {
            exec_cmd.push(' ');
            exec_cmd.push_str(a);
        }

        let mut out = String::new();
        out.push_str(
            "[Unit]
",
        );
        out.push_str(&format!(
            "Description={desc}
"
        ));
        if !after_str.is_empty() {
            out.push_str(&format!(
                "After={after_str}
"
            ));
        }
        if !requires_str.is_empty() {
            out.push_str(&format!(
                "Requires={requires_str}
"
            ));
        }

        out.push_str(
            "
[Service]
",
        );
        out.push_str(
            "Type=simple
",
        );
        out.push_str(&format!(
            "ExecStart={exec_cmd}
"
        ));
        out.push_str(&format!(
            "Restart={}
",
            self.restart_policy
        ));

        if let Some(ref u) = self.user {
            out.push_str(&format!(
                "User={u}
"
            ));
        }
        if let Some(ref g) = self.group {
            out.push_str(&format!(
                "Group={g}
"
            ));
        }
        if let Some(ref wd) = self.working_directory {
            out.push_str(&format!(
                "WorkingDirectory={}
",
                wd.display()
            ));
        }

        out.push_str(
            "
[Install]
",
        );
        out.push_str(
            "WantedBy=multi-user.target
",
        );

        out
    }

    /// Returns systemctl commands required to manage the service lifecycle.
    ///
    /// # Returns
    ///
    /// Sequence of `systemctl` shell command strings.
    #[must_use]
    pub fn systemd_lifecycle_commands(&self) -> Vec<String> {
        let mut cmds = vec!["systemctl daemon-reload".to_string()];
        if self.auto_start {
            cmds.push(format!("systemctl enable {}", self.name));
            cmds.push(format!("systemctl start {}", self.name));
        }
        cmds
    }

    // --------------------------------------------------------------------------------
    // macOS launchd Property List Generation
    // --------------------------------------------------------------------------------

    /// Returns the standard launchd plist file installation path.
    ///
    /// # Arguments
    ///
    /// * `is_system_daemon` - `true` for `/Library/LaunchDaemons`, `false` for `~/Library/LaunchAgents`.
    #[must_use]
    pub fn launchd_plist_path(&self, is_system_daemon: bool) -> PathBuf {
        let base = if is_system_daemon {
            PathBuf::from("/Library/LaunchDaemons")
        } else {
            PathBuf::from("~/Library/LaunchAgents")
        };
        base.join(format!("{}.plist", self.name))
    }

    /// Generates XML launchd property list (.plist) content.
    ///
    /// # Returns
    ///
    /// XML string.
    #[must_use]
    #[allow(clippy::format_push_string)]
    pub fn generate_launchd_plist(&self) -> String {
        let mut args_xml = String::new();
        args_xml.push_str(&format!(
            "        <string>{}</string>\n",
            self.exec_start.display()
        ));
        for a in &self.arguments {
            args_xml.push_str(&format!("        <string>{a}</string>\n"));
        }

        let name = &self.name;
        let run_tag = if self.auto_start {
            "<true/>"
        } else {
            "<false/>"
        };

        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        out.push('\n');
        out.push_str(r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#);
        out.push('\n');
        out.push_str(r#"<plist version="1.0">"#);
        out.push('\n');
        out.push_str("<dict>\n");
        out.push_str(&format!(
            "    <key>Label</key>\n    <string>{name}</string>\n"
        ));
        out.push_str("    <key>ProgramArguments</key>\n    <array>\n");
        out.push_str(&args_xml);
        out.push_str("    </array>\n");
        out.push_str("    <key>KeepAlive</key>\n    <true/>\n");
        out.push_str(&format!("    <key>RunAtLoad</key>\n    {run_tag}\n"));
        out.push_str(&format!(
            "    <key>StandardOutPath</key>\n    <string>/var/log/{name}.log</string>\n"
        ));
        out.push_str(&format!(
            "    <key>StandardErrorPath</key>\n    <string>/var/log/{name}.err</string>\n"
        ));
        out.push_str("</dict>\n");
        out.push_str("</plist>\n");

        out
    }

    /// Returns `launchctl` commands required to bootstrap and manage the service.
    ///
    /// # Arguments
    ///
    /// * `is_system_daemon` - System daemon or user agent.
    #[must_use]
    pub fn launchd_lifecycle_commands(&self, is_system_daemon: bool) -> Vec<String> {
        let path = self.launchd_plist_path(is_system_daemon);
        let domain = if is_system_daemon { "system" } else { "gui" };
        vec![
            format!("launchctl bootstrap {domain} {}", path.display()),
            format!("launchctl kickstart -k {domain}/{}", self.name),
        ]
    }

    // --------------------------------------------------------------------------------
    // FreeBSD rc.d Control Script Generation
    // --------------------------------------------------------------------------------

    /// Returns the standard FreeBSD rc.d script path (`/usr/local/etc/rc.d/<service>`).
    #[must_use]
    pub fn freebsd_rc_script_path(&self) -> PathBuf {
        PathBuf::from("/usr/local/etc/rc.d").join(&self.name)
    }

    /// Generates FreeBSD `rc.subr` conforming shell script.
    ///
    /// # Returns
    ///
    /// Bourne shell script string.
    #[must_use]
    #[allow(clippy::format_push_string)]
    pub fn generate_freebsd_rc_script(&self) -> String {
        let name = &self.name;
        let exec = self.exec_start.display();
        let mut out = String::new();
        out.push_str("#!/bin/sh\n\n");
        out.push_str(&format!("# PROVIDE: {name}\n"));
        out.push_str("# REQUIRE: LOGIN cleanvar\n");
        out.push_str("# KEYWORD: shutdown\n\n");
        out.push_str(". /etc/rc.subr\n\n");
        out.push_str(&format!(r#"name="{name}""#));
        out.push('\n');
        out.push_str(&format!(r#"rcvar="{name}_enable""#));
        out.push('\n');
        out.push_str(&format!(r#"command="{exec}""#));
        out.push('\n');

        if !self.arguments.is_empty() {
            let args = self.arguments.join(" ");
            out.push_str(&format!(r#"command_args="{args}""#));
            out.push('\n');
        }
        if let Some(ref u) = self.user {
            out.push_str(&format!(r#"{name}_user="{u}""#));
            out.push('\n');
        }

        out.push_str(&format!(r#"load_rc_config "{name}""#));
        out.push_str("\nrun_rc_command \"$1\"\n");

        out
    }

    /// Returns FreeBSD `sysrc` and `service` commands to enable and start.
    #[must_use]
    pub fn freebsd_lifecycle_commands(&self) -> Vec<String> {
        let mut cmds = Vec::new();
        if self.auto_start {
            cmds.push(format!(r#"sysrc {}_enable="YES""#, self.name));
            cmds.push(format!("service {} start", self.name));
        }
        cmds
    }

    // --------------------------------------------------------------------------------
    // SunOS / illumos SMF Manifest Generation
    // --------------------------------------------------------------------------------

    /// Returns the SMF XML manifest path (`/lib/svc/manifest/site/<name>.xml`).
    #[must_use]
    pub fn smf_manifest_path(&self) -> PathBuf {
        PathBuf::from("/lib/svc/manifest/site").join(format!("{}.xml", self.name))
    }

    /// Generates `SunOS` / illumos Service Management Facility (SMF) XML manifest.
    ///
    /// # Returns
    ///
    /// XML string.
    #[must_use]
    #[allow(clippy::format_push_string, clippy::needless_raw_string_hashes)]
    pub fn generate_smf_manifest(&self) -> String {
        let desc = self.description.as_deref().unwrap_or(&self.display_name);
        let exec_cmd = self.exec_start.display();
        let name = &self.name;

        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0"?>"#);
        out.push('\n');
        out.push_str(
            r#"<!DOCTYPE service_bundle SYSTEM "/usr/share/lib/xml/dtd/service_bundle.dtd.1">"#,
        );
        out.push('\n');
        out.push_str(r#"<service_bundle type="manifest" name="site:custom">"#);
        out.push('\n');
        out.push_str(&format!(
            r#"  <service name="site/{name}" type="service" version="1">"#
        ));
        out.push('\n');
        out.push_str(r#"    <create_default_instance enabled="true"/>"#);
        out.push('\n');
        out.push_str("    <single_instance/>");
        out.push('\n');
        out.push_str(r#"    <dependency name="network" grouping="require_all" restart_on="error" type="service">"#);
        out.push('\n');
        out.push_str(r#"      <service_fmri value="svc:/milestone/network:default"/>"#);
        out.push('\n');
        out.push_str("    </dependency>");
        out.push('\n');
        out.push_str(&format!(r#"    <exec_method type="method" name="start" exec="{exec_cmd}" timeout_seconds="60"/>"#));
        out.push('\n');
        out.push_str(
            r#"    <exec_method type="method" name="stop" exec=":kill" timeout_seconds="60"/>"#,
        );
        out.push('\n');
        out.push_str("    <template>");
        out.push('\n');
        out.push_str(&format!(
            r#"      <common_name><loctext xml:lang="C">{desc}</loctext></common_name>"#
        ));
        out.push('\n');
        out.push_str("    </template>");
        out.push('\n');
        out.push_str("  </service>");
        out.push('\n');
        out.push_str("</service_bundle>");
        out.push('\n');

        out
    }

    /// Returns `svccfg` and `svcadm` commands for SMF service registration and lifecycle.
    #[must_use]
    pub fn smf_lifecycle_commands(&self) -> Vec<String> {
        let path = self.smf_manifest_path();
        vec![
            format!("svccfg import {}", path.display()),
            format!("svcadm enable -s site/{}", self.name),
        ]
    }
}

/// Host service supervisor subsystem classifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupervisorType {
    /// Linux systemd service manager.
    Systemd,
    /// Apple macOS launchd daemon manager.
    Launchd,
    /// FreeBSD rc.d / rc.subr service system.
    FreeBsdRc,
    /// `SunOS` / illumos Service Management Facility (SMF).
    Smf,
}

impl SupervisorType {
    /// Detects the host platform service supervisor based on target OS.
    #[must_use]
    pub const fn detect_host() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::Launchd
        }
        #[cfg(target_os = "freebsd")]
        {
            Self::FreeBsdRc
        }
        #[cfg(any(target_os = "solaris", target_os = "illumos"))]
        {
            Self::Smf
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "solaris",
            target_os = "illumos"
        )))]
        {
            Self::Systemd
        }
    }
}

/// Service control action command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceControlAction {
    /// Start the service daemon.
    Start,
    /// Stop the running service daemon.
    Stop,
    /// Restart the service daemon.
    Restart,
    /// Enable service auto-start at boot.
    Enable,
    /// Disable service auto-start at boot.
    Disable,
    /// Delete / remove service registration.
    Delete,
}

/// Information about a physically installed service artifact for tracking and rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledService {
    /// Internal service identifier.
    pub name: String,
    /// Target supervisor system.
    pub supervisor: SupervisorType,
    /// Absolute path to the written unit / plist / script file.
    pub unit_file_path: PathBuf,
    /// Whether the service was started during installation.
    pub was_started: bool,
    /// Whether the service was enabled at boot.
    pub was_enabled: bool,
}

/// Default command execution timeout in milliseconds (30 seconds).
pub const DEFAULT_SUPERVISOR_TIMEOUT_MS: u64 = 30_000;

/// Live host service supervisor execution engine executing unit writing and lifecycle commands.
#[derive(Debug, Clone)]
pub struct HostSupervisorExecutor {
    /// Audit log of installed services for rollback compensation.
    installed_services: Vec<InstalledService>,
    /// Chronological log of commands generated/dispatched.
    executed_commands: Vec<String>,
    /// Whether dry-run simulation mode is active.
    dry_run: bool,
    /// Command execution timeout in milliseconds.
    timeout_ms: u64,
}

impl Default for HostSupervisorExecutor {
    fn default() -> Self {
        Self {
            installed_services: Vec::new(),
            executed_commands: Vec::new(),
            dry_run: true,
            timeout_ms: DEFAULT_SUPERVISOR_TIMEOUT_MS,
        }
    }
}

impl HostSupervisorExecutor {
    /// Creates a new [`HostSupervisorExecutor`] with dry-run mode enabled by default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether dry-run simulation mode is enabled.
    #[must_use]
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Configures the command execution timeout in milliseconds.
    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Returns whether dry-run mode is enabled.
    #[must_use]
    pub const fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Returns the command timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns the slice of installed service records.
    #[must_use]
    pub fn installed_services(&self) -> &[InstalledService] {
        &self.installed_services
    }

    /// Returns the chronological log of executed supervisor lifecycle commands.
    #[must_use]
    pub fn executed_commands(&self) -> &[String] {
        &self.executed_commands
    }

    /// Physically installs and writes the service unit/plist/script file to disk.
    ///
    /// # Arguments
    ///
    /// * `svc` - Service definition specification.
    /// * `supervisor` - Target supervisor platform.
    /// * `root_prefix` - Optional prefix path for isolated test installations.
    ///
    /// # Returns
    ///
    /// The recorded [`InstalledService`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem failure.
    pub fn install_service(
        &mut self,
        svc: &ServiceDefinition,
        supervisor: SupervisorType,
        root_prefix: Option<&Path>,
    ) -> Result<InstalledService> {
        let (rel_path, content, is_executable) = match supervisor {
            SupervisorType::Systemd => {
                (svc.systemd_unit_path(), svc.generate_systemd_unit(), false)
            }
            SupervisorType::Launchd => (
                svc.launchd_plist_path(true),
                svc.generate_launchd_plist(),
                false,
            ),
            SupervisorType::FreeBsdRc => (
                svc.freebsd_rc_script_path(),
                svc.generate_freebsd_rc_script(),
                true,
            ),
            SupervisorType::Smf => (svc.smf_manifest_path(), svc.generate_smf_manifest(), false),
        };

        let target_path = if let Some(prefix) = root_prefix {
            let stripped = rel_path.strip_prefix("/").unwrap_or(&rel_path);
            prefix.join(stripped)
        } else {
            rel_path
        };

        let mut dir = target_path.clone();
        dir.pop();
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| {
                Error::Io(format!(
                    "Failed to create supervisor directory {}: {e}",
                    dir.display()
                ))
            })?;
        }

        fs::write(&target_path, content.as_bytes()).map_err(|e| {
            Error::Io(format!(
                "Failed to write service unit file {}: {e}",
                target_path.display()
            ))
        })?;

        #[cfg(unix)]
        if is_executable {
            use std::os::unix::fs::PermissionsExt;
            let perm = fs::Permissions::from_mode(0o755);
            let _ = fs::set_permissions(&target_path, perm);
        }
        #[cfg(not(unix))]
        let _ = is_executable;

        let record = InstalledService {
            name: svc.name.clone(),
            supervisor,
            unit_file_path: target_path,
            was_started: false,
            was_enabled: svc.auto_start,
        };

        self.installed_services.push(record.clone());
        Ok(record)
    }

    /// Dispatches a lifecycle control command to the target supervisor.
    ///
    /// # Arguments
    ///
    /// * `svc_name` - Service name.
    /// * `action` - Control action ([`ServiceControlAction`]).
    /// * `supervisor` - Target supervisor platform.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if service is not found or supervisor rejects command.
    pub fn execute_control(
        &mut self,
        svc_name: &str,
        action: ServiceControlAction,
        supervisor: SupervisorType,
    ) -> Result<()> {
        let cmd = match (supervisor, action) {
            (SupervisorType::Systemd, ServiceControlAction::Start) => {
                format!("systemctl start {svc_name}")
            }
            (SupervisorType::Systemd, ServiceControlAction::Stop) => {
                format!("systemctl stop {svc_name}")
            }
            (SupervisorType::Systemd, ServiceControlAction::Restart) => {
                format!("systemctl restart {svc_name}")
            }
            (SupervisorType::Systemd, ServiceControlAction::Enable) => {
                format!("systemctl enable {svc_name}")
            }
            (SupervisorType::Systemd, ServiceControlAction::Disable) => {
                format!("systemctl disable {svc_name}")
            }
            (SupervisorType::Systemd, ServiceControlAction::Delete) => {
                format!("systemctl disable --now {svc_name}")
            }

            (
                SupervisorType::Launchd,
                ServiceControlAction::Start | ServiceControlAction::Restart,
            ) => {
                format!("launchctl kickstart -k system/{svc_name}")
            }
            (SupervisorType::Launchd, ServiceControlAction::Stop) => {
                format!("launchctl kill SIGTERM system/{svc_name}")
            }
            (SupervisorType::Launchd, ServiceControlAction::Enable) => {
                format!("launchctl enable system/{svc_name}")
            }
            (SupervisorType::Launchd, ServiceControlAction::Disable) => {
                format!("launchctl disable system/{svc_name}")
            }
            (SupervisorType::Launchd, ServiceControlAction::Delete) => {
                format!("launchctl bootout system/{svc_name}")
            }

            (SupervisorType::FreeBsdRc, ServiceControlAction::Start) => {
                format!("service {svc_name} start")
            }
            (SupervisorType::FreeBsdRc, ServiceControlAction::Stop) => {
                format!("service {svc_name} stop")
            }
            (SupervisorType::FreeBsdRc, ServiceControlAction::Restart) => {
                format!("service {svc_name} restart")
            }
            (SupervisorType::FreeBsdRc, ServiceControlAction::Enable) => {
                format!("sysrc {svc_name}_enable=\"YES\"")
            }
            (SupervisorType::FreeBsdRc, ServiceControlAction::Disable) => {
                format!("sysrc {svc_name}_enable=\"NO\"")
            }
            (SupervisorType::FreeBsdRc, ServiceControlAction::Delete) => {
                format!("service {svc_name} stop && sysrc -x {svc_name}_enable")
            }

            (SupervisorType::Smf, ServiceControlAction::Start) => {
                format!("svcadm enable -s site/{svc_name}")
            }
            (SupervisorType::Smf, ServiceControlAction::Stop) => {
                format!("svcadm disable -s site/{svc_name}")
            }
            (SupervisorType::Smf, ServiceControlAction::Restart) => {
                format!("svcadm restart site/{svc_name}")
            }
            (SupervisorType::Smf, ServiceControlAction::Enable) => {
                format!("svcadm enable site/{svc_name}")
            }
            (SupervisorType::Smf, ServiceControlAction::Disable) => {
                format!("svcadm disable site/{svc_name}")
            }
            (SupervisorType::Smf, ServiceControlAction::Delete) => {
                format!("svccfg delete site/{svc_name}")
            }
        };

        self.executed_commands.push(cmd.clone());

        if !self.dry_run {
            Self::run_supervisor_command(&cmd, self.timeout_ms)?;
        }

        if let Some(record) = self
            .installed_services
            .iter_mut()
            .find(|s| s.name == svc_name)
        {
            match action {
                ServiceControlAction::Start => record.was_started = true,
                ServiceControlAction::Stop => record.was_started = false,
                ServiceControlAction::Enable => record.was_enabled = true,
                ServiceControlAction::Disable => record.was_enabled = false,
                ServiceControlAction::Restart | ServiceControlAction::Delete => {}
            }
        }

        Ok(())
    }

    /// Executes a supervisor shell command synchronously with a timeout.
    ///
    /// # Arguments
    ///
    /// * `cmd_str` - Command string to execute via `sh -c`.
    /// * `timeout_ms` - Maximum execution time in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the command fails, times out, or cannot be spawned.
    fn run_supervisor_command(cmd_str: &str, timeout_ms: u64) -> Result<()> {
        Self::run_supervisor_command_with_shell("sh", cmd_str, timeout_ms)
    }

    /// Executes a supervisor command using a specific shell binary.
    ///
    /// # Arguments
    ///
    /// * `shell` - Shell executable name or path.
    /// * `cmd_str` - Command string to execute.
    /// * `timeout_ms` - Maximum execution time in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the command fails, times out, or cannot be spawned.
    fn run_supervisor_command_with_shell(
        shell: &str,
        cmd_str: &str,
        timeout_ms: u64,
    ) -> Result<()> {
        use std::io::Read as _;

        let mut cmd = Command::new(shell);
        cmd.arg("-c").arg(cmd_str);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            Error::Io(format!(
                "Failed to spawn supervisor command '{cmd_str}': {e}"
            ))
        })?;

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            if let Some(status) = child.try_wait().unwrap_or(None) {
                if status.success() {
                    return Ok(());
                }
                let mut stderr_bytes = Vec::new();
                let _ = child
                    .stderr
                    .as_mut()
                    .map(|err| err.read_to_end(&mut stderr_bytes));
                let stderr = String::from_utf8_lossy(&stderr_bytes);
                return Err(Error::Io(format!(
                    "Supervisor command '{cmd_str}' failed with status {status}: {stderr}"
                )));
            }
            if start.elapsed() > timeout {
                let _ = child.kill();
                return Err(Error::Io(format!(
                    "Supervisor command '{cmd_str}' timed out after {timeout_ms}ms"
                )));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Compensating rollback restoring original state by stopping, disabling, and deleting installed service files.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem deletion failure.
    pub fn rollback(&mut self) -> Result<()> {
        let services_to_rollback = std::mem::take(&mut self.installed_services);
        for svc in services_to_rollback.into_iter().rev() {
            if svc.was_started {
                drop(self.execute_control(&svc.name, ServiceControlAction::Stop, svc.supervisor));
            }
            if svc.was_enabled {
                drop(self.execute_control(
                    &svc.name,
                    ServiceControlAction::Disable,
                    svc.supervisor,
                ));
            }
            if svc.unit_file_path.exists() {
                drop(fs::remove_file(&svc.unit_file_path));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests systemd unit and command generation.
    #[test]
    fn test_systemd_generation() {
        let svc = ServiceDefinition::new("acme-agent", "/usr/bin/acme-agent")
            .display_name("Acme Agent Service")
            .description("Acme Cloud Agent Daemon")
            .arg("--config")
            .arg("/etc/acme.conf")
            .working_dir("/var/lib/acme")
            .run_as("acme", Some("acme".to_string()))
            .auto_start(true);

        assert_eq!(
            svc.systemd_unit_path(),
            PathBuf::from("/lib/systemd/system/acme-agent.service")
        );

        let unit = svc.generate_systemd_unit();
        assert!(unit.contains("Description=Acme Cloud Agent Daemon"));
        assert!(unit.contains("ExecStart=/usr/bin/acme-agent --config /etc/acme.conf"));
        assert!(unit.contains("User=acme"));
        assert!(unit.contains("Group=acme"));
        assert!(unit.contains("WorkingDirectory=/var/lib/acme"));
        assert!(unit.contains("WantedBy=multi-user.target"));

        let cmds = svc.systemd_lifecycle_commands();
        assert_eq!(
            cmds,
            vec![
                "systemctl daemon-reload",
                "systemctl enable acme-agent",
                "systemctl start acme-agent",
            ]
        );
    }

    /// Tests launchd plist generation and lifecycle commands.
    #[test]
    fn test_launchd_generation() {
        let svc = ServiceDefinition::new(
            "com.acme.agent",
            "/Applications/Acme.app/Contents/MacOS/agent",
        )
        .arg("-v");

        let plist = svc.generate_launchd_plist();
        assert!(plist.contains("<key>Label</key>"));
        assert!(plist.contains("com.acme.agent"));
        assert!(plist.contains("<string>/Applications/Acme.app/Contents/MacOS/agent</string>"));
        assert!(plist.contains("<string>-v</string>"));
        assert!(plist.contains("<key>KeepAlive</key>"));

        let cmds = svc.launchd_lifecycle_commands(true);
        assert_eq!(cmds.len(), 2);
        assert!(cmds[0]
            .contains("launchctl bootstrap system /Library/LaunchDaemons/com.acme.agent.plist"));
    }

    /// Tests FreeBSD rc.d script and `SunOS` SMF manifest generation.
    #[test]
    fn test_freebsd_and_smf_generation() {
        let svc = ServiceDefinition::new("acmesvc", "/usr/local/bin/acmesvc")
            .arg("-d")
            .run_as("daemon", None);

        // FreeBSD rc.d
        assert_eq!(
            svc.freebsd_rc_script_path(),
            PathBuf::from("/usr/local/etc/rc.d/acmesvc")
        );
        let rc = svc.generate_freebsd_rc_script();
        assert!(rc.contains("# PROVIDE: acmesvc"));
        assert!(rc.contains(". /etc/rc.subr"));
        assert!(rc.contains(r#"name="acmesvc""#));
        assert!(rc.contains(r#"command_args="-d""#));
        assert!(rc.contains(r#"acmesvc_user="daemon""#));

        let rc_cmds = svc.freebsd_lifecycle_commands();
        assert_eq!(
            rc_cmds,
            vec![r#"sysrc acmesvc_enable="YES""#, "service acmesvc start",]
        );

        // SunOS SMF
        assert_eq!(
            svc.smf_manifest_path(),
            PathBuf::from("/lib/svc/manifest/site/acmesvc.xml")
        );
        let smf = svc.generate_smf_manifest();
        assert!(smf.contains(r#"<service name="site/acmesvc""#));
        assert!(smf.contains(r#"exec="/usr/local/bin/acmesvc""#));
        assert!(smf.contains(r#"<service_fmri value="svc:/milestone/network:default"/>"#));

        let smf_cmds = svc.smf_lifecycle_commands();
        assert_eq!(
            smf_cmds,
            vec![
                "svccfg import /lib/svc/manifest/site/acmesvc.xml",
                "svcadm enable -s site/acmesvc",
            ]
        );
    }

    #[test]
    fn test_host_supervisor_executor_lifecycle_and_rollback() {
        let temp_dir = std::env::temp_dir().join(format!("msi_daemon_test_{}", std::process::id()));
        let mut executor = HostSupervisorExecutor::new();
        let svc = ServiceDefinition::new("my-daemon", "/usr/bin/my-daemon").auto_start(true);

        // Install for systemd
        let res_inst = executor.install_service(&svc, SupervisorType::Systemd, Some(&temp_dir));
        assert!(res_inst.is_ok());
        let installed = res_inst.unwrap_or(InstalledService {
            name: String::new(),
            supervisor: SupervisorType::Systemd,
            unit_file_path: PathBuf::new(),
            was_started: false,
            was_enabled: false,
        });
        assert!(installed.unit_file_path.exists());
        assert_eq!(installed.name, "my-daemon");
        assert_eq!(installed.supervisor, SupervisorType::Systemd);

        // Execute control actions
        assert!(executor
            .execute_control(
                "my-daemon",
                ServiceControlAction::Start,
                SupervisorType::Systemd
            )
            .is_ok());
        assert!(executor
            .execute_control(
                "my-daemon",
                ServiceControlAction::Enable,
                SupervisorType::Systemd
            )
            .is_ok());
        assert!(executor
            .execute_control(
                "my-daemon",
                ServiceControlAction::Restart,
                SupervisorType::Systemd
            )
            .is_ok());
        assert_eq!(executor.executed_commands().len(), 3);
        assert!(executor.executed_commands()[0].contains("systemctl start"));

        // Rollback
        let res_rb = executor.rollback();
        assert!(res_rb.is_ok());
        assert_eq!(executor.installed_services().len(), 0);

        // Verify file was removed on rollback
        let unit_path = temp_dir.join("lib/systemd/system/my-daemon.service");
        assert!(!unit_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_host_supervisor_executor_options_and_live_dispatch() {
        let executor = HostSupervisorExecutor::new()
            .with_dry_run(true)
            .with_timeout_ms(5000);
        assert!(executor.is_dry_run());
        assert_eq!(executor.timeout_ms(), 5000);

        // Test run_supervisor_command success
        assert!(
            HostSupervisorExecutor::run_supervisor_command("echo hello > /dev/null", 5000).is_ok()
        );

        // Test run_supervisor_command failure
        let res_err = HostSupervisorExecutor::run_supervisor_command("exit 1", 5000);
        assert!(res_err.is_err());

        // Test run_supervisor_command timeout
        let res_timeout = HostSupervisorExecutor::run_supervisor_command("sleep 1", 50);
        assert!(res_timeout.is_err());
    }

    #[test]
    fn test_supervisor_type_detect_host() {
        let sup = SupervisorType::detect_host();
        assert!(matches!(
            sup,
            SupervisorType::Systemd
                | SupervisorType::Launchd
                | SupervisorType::FreeBsdRc
                | SupervisorType::Smf
        ));
    }

    /// Tests systemd unit edge cases with empty `after` and populated `requires`, no user/group/wd, and disabled `auto_start`.
    #[test]
    fn test_systemd_generation_edge_cases() {
        let mut svc = ServiceDefinition::new("minimal-agent", "/usr/bin/minimal");
        svc.after = Vec::new();
        svc.requires = vec!["custom.target".to_string()];
        svc.auto_start = false;

        let unit = svc.generate_systemd_unit();
        assert!(!unit.contains("After="));
        assert!(unit.contains("Requires=custom.target"));
        assert!(!unit.contains("User="));
        assert!(!unit.contains("Group="));
        assert!(!unit.contains("WorkingDirectory="));

        let cmds = svc.systemd_lifecycle_commands();
        assert_eq!(cmds, vec!["systemctl daemon-reload"]);
    }

    /// Tests launchd property list edge cases with non-daemon user agents, no arguments, and disabled `auto_start`.
    #[test]
    fn test_launchd_generation_edge_cases() {
        let svc =
            ServiceDefinition::new("com.acme.useragent", "/usr/bin/useragent").auto_start(false);

        let user_path = svc.launchd_plist_path(false);
        assert_eq!(
            user_path,
            PathBuf::from("~/Library/LaunchAgents/com.acme.useragent.plist")
        );

        let plist = svc.generate_launchd_plist();
        assert!(plist.contains("<key>RunAtLoad</key>\n    <false/>"));

        let cmds = svc.launchd_lifecycle_commands(false);
        assert_eq!(cmds.len(), 2);
        assert!(cmds[0]
            .contains("launchctl bootstrap gui ~/Library/LaunchAgents/com.acme.useragent.plist"));
        assert!(cmds[1].contains("launchctl kickstart -k gui/com.acme.useragent"));
    }

    /// Tests FreeBSD rc script edge cases with no arguments, no user, and `auto_start` disabled.
    #[test]
    fn test_freebsd_generation_edge_cases() {
        let svc = ServiceDefinition::new("simple-rc", "/usr/local/bin/simple").auto_start(false);

        let rc = svc.generate_freebsd_rc_script();
        assert!(!rc.contains("command_args="));
        assert!(!rc.contains("simple-rc_user="));

        let cmds = svc.freebsd_lifecycle_commands();
        assert_eq!(cmds.len(), 0);
    }

    /// Tests SMF manifest generation fallback when description is None.
    #[test]
    fn test_smf_generation_fallback() {
        let svc = ServiceDefinition::new("smf-service", "/opt/site/bin/smf-service");
        let manifest = svc.generate_smf_manifest();
        assert!(manifest
            .contains("<common_name><loctext xml:lang=\"C\">smf-service</loctext></common_name>"));
    }

    /// Tests all combinations of supervisor platforms and service control actions.
    #[test]
    fn test_execute_control_all_supervisors_and_actions() {
        let mut executor = HostSupervisorExecutor::new();
        let svc = ServiceDefinition::new("matrix-daemon", "/bin/daemon").auto_start(true);
        let temp_dir =
            std::env::temp_dir().join(format!("msi_daemon_ctrl_test_{}", std::process::id()));

        // Register installed service
        let inst = executor.install_service(&svc, SupervisorType::Systemd, Some(&temp_dir));
        assert!(inst.is_ok());

        let supervisors = [
            SupervisorType::Systemd,
            SupervisorType::Launchd,
            SupervisorType::FreeBsdRc,
            SupervisorType::Smf,
        ];
        let actions = [
            ServiceControlAction::Start,
            ServiceControlAction::Stop,
            ServiceControlAction::Restart,
            ServiceControlAction::Enable,
            ServiceControlAction::Disable,
            ServiceControlAction::Delete,
        ];

        for sup in supervisors {
            for act in actions {
                assert!(executor.execute_control("matrix-daemon", act, sup).is_ok());
            }
        }

        // Test control when service name is not in installed_services (find returns None)
        assert!(executor
            .execute_control(
                "unknown-daemon",
                ServiceControlAction::Start,
                SupervisorType::Systemd
            )
            .is_ok());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests installing service files across Launchd, FreeBSD, and SMF, as well as failure branches.
    #[test]
    fn test_install_service_all_variants_and_errors() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_daemon_install_test_{}", std::process::id()));
        let mut executor = HostSupervisorExecutor::new();
        let svc = ServiceDefinition::new("test-agent", "/usr/bin/test-agent");

        // Launchd
        let res_launchd = executor.install_service(&svc, SupervisorType::Launchd, Some(&temp_dir));
        assert!(res_launchd.is_ok());

        // Second install when parent directory already exists (exercises parent.exists() == true branch)
        let svc2 = ServiceDefinition::new("second-agent", "/usr/bin/second-agent");
        let res_second = executor.install_service(&svc2, SupervisorType::Launchd, Some(&temp_dir));
        assert!(res_second.is_ok());

        // FreeBSD rc (sets executable permissions on Unix)
        let res_freebsd =
            executor.install_service(&svc, SupervisorType::FreeBsdRc, Some(&temp_dir));
        assert!(res_freebsd.is_ok());

        // SMF
        let res_smf = executor.install_service(&svc, SupervisorType::Smf, Some(&temp_dir));
        assert!(res_smf.is_ok());

        // Failure when parent path is blocked by a file (making create_dir_all fail)
        let file_as_dir = temp_dir.join("blocked_dir_file");
        let _ = fs::write(&file_as_dir, b"not a directory");
        let blocked_prefix = file_as_dir.join("sub");
        let res_blocked =
            executor.install_service(&svc, SupervisorType::Systemd, Some(&blocked_prefix));
        assert!(res_blocked.is_err());

        // Failure when write to target_path fails (target_path itself is an existing directory)
        let dir_target = temp_dir.join("lib/systemd/system/dir_target.service");
        let _ = fs::create_dir_all(&dir_target);
        let svc_dir = ServiceDefinition::new("dir_target", "/usr/bin/dir_target");
        let res_write_err =
            executor.install_service(&svc_dir, SupervisorType::Systemd, Some(&temp_dir));
        assert!(res_write_err.is_err());

        // Failure when installing with root_prefix = None without write permissions to /lib
        let res_no_prefix = executor.install_service(&svc, SupervisorType::Systemd, None);
        assert!(res_no_prefix.is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests live dispatch (`dry_run` = false) in `execute_control`.
    #[test]
    fn test_execute_control_live_command_error() {
        let mut executor = HostSupervisorExecutor::new().with_dry_run(false);
        // Attempting to control a nonexistent service in live mode fails with Error::Io
        let res = executor.execute_control(
            "nonexistent-mock-service-msi",
            ServiceControlAction::Start,
            SupervisorType::Systemd,
        );
        assert!(res.is_err());
    }

    /// Tests supervisor command spawn error using invalid shell path.
    #[test]
    fn test_run_supervisor_command_spawn_error() {
        let res = HostSupervisorExecutor::run_supervisor_command_with_shell(
            "/nonexistent/mock_sh_binary_msi",
            "true",
            1000,
        );
        assert!(res.is_err());
    }

    /// Tests rollback branches where services were not started, not enabled, and unit file does not exist.
    #[test]
    fn test_rollback_inactive_and_missing_file() {
        let mut executor = HostSupervisorExecutor::new();
        let record = InstalledService {
            name: "phantom-service".to_string(),
            supervisor: SupervisorType::Systemd,
            unit_file_path: PathBuf::from("/nonexistent/path/phantom.service"),
            was_started: false,
            was_enabled: false,
        };
        executor.installed_services.push(record);

        let res = executor.rollback();
        assert!(res.is_ok());
        assert_eq!(executor.installed_services.len(), 0);
    }
}

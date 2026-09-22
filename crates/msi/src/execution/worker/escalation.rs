//! Privilege Boundary Separation and Escalation Handlers.
//!
//! Grounded in platform security standards for elevating installer privileges:
//! - **Windows**: Elevated worker execution via `ShellExecuteExW` with `runas` verb
//!   or connecting to the Windows Installer service (`msiserver`).
//! - **Linux / POSIX**: Privilege boundary transition via `PolicyKit` (`pkexec`),
//!   `sudo`, or systemd transient service unit (`systemd-run --wait -t`).
//! - **macOS**: Privileged helper tool execution via Apple Authorization Services
//!   (`SMJobBless`).
//! - **Direct**: In-process or already-privileged worker execution.

use std::collections::HashMap;

/// Supported privilege escalation strategies across operating systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EscalationMethod {
    /// Direct execution without escalation (already privileged, test mode, or non-admin user).
    Direct,
    /// Unix `sudo` privilege escalation.
    Sudo,
    /// Freedesktop `PolicyKit` `pkexec` privilege escalation for graphical / interactive sessions.
    PkExec,
    /// Systemd transient service unit execution (`systemd-run --wait -t`).
    SystemdRun,
    /// macOS Apple Authorization Services privileged helper tool (`SMJobBless`).
    SMJobBless,
    /// macOS graphical `AppleScript` administrator prompt (`osascript -e 'do shell script "..." with administrator privileges'`).
    OsascriptAdmin,
    /// Windows `ShellExecute` elevation using the `runas` verb.
    ShellExecuteRunAs,
}

/// Specification of a command to be spawned by the host operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Target executable path or command name.
    pub program: String,
    /// Command-line argument vector.
    pub args: Vec<String>,
    /// Environment variables to inject or pass into the process.
    pub env: HashMap<String, String>,
}

/// Factory constructing platform-specific privileged worker invocation commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivilegeEscalator;

impl PrivilegeEscalator {
    /// Detects the recommended privilege escalation method for the current host environment.
    ///
    /// # Returns
    ///
    /// Recommended [`EscalationMethod`].
    #[must_use]
    pub fn detect_best_method() -> EscalationMethod {
        #[cfg(windows)]
        {
            EscalationMethod::ShellExecuteRunAs
        }
        #[cfg(target_os = "macos")]
        {
            // If running in terminal / SSH session, use sudo; otherwise graphical osascript
            if std::env::var_os("SSH_CONNECTION").is_some()
                || std::env::var_os("TERM_PROGRAM").is_some()
            {
                EscalationMethod::Sudo
            } else {
                EscalationMethod::OsascriptAdmin
            }
        }
        #[cfg(target_os = "linux")]
        {
            // Check if running in a graphical X11/Wayland desktop session with pkexec
            if std::env::var_os("DISPLAY").is_some()
                || std::env::var_os("WAYLAND_DISPLAY").is_some()
            {
                EscalationMethod::PkExec
            } else {
                EscalationMethod::Sudo
            }
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            EscalationMethod::Sudo
        }
    }

    /// Detects the recommended escalation method for macOS environments.
    ///
    /// # Arguments
    ///
    /// * `has_gui` - Whether an interactive desktop graphical session is active.
    /// * `helper_blessed` - Whether a privileged helper tool is already registered via `SMJobBless`.
    ///
    /// # Returns
    ///
    /// Selected [`EscalationMethod`].
    #[must_use]
    pub const fn detect_macos_method(has_gui: bool, helper_blessed: bool) -> EscalationMethod {
        if helper_blessed {
            EscalationMethod::SMJobBless
        } else if has_gui {
            EscalationMethod::OsascriptAdmin
        } else {
            EscalationMethod::Sudo
        }
    }

    /// Generates a launchd job property list (`.plist`) XML for an `SMJobBless` privileged helper daemon.
    ///
    /// # Arguments
    ///
    /// * `job_label` - Reverse-DNS identifier of the privileged helper tool (e.g. `com.example.msi.helper`).
    /// * `executable_path` - Path to the installed helper binary in `/Library/PrivilegedHelperTools`.
    /// * `socket_path` - Path to the IPC socket or named pipe.
    ///
    /// # Returns
    ///
    /// Formatted XML property list string.
    #[must_use]
    pub fn generate_smjobbless_plist(
        job_label: &str,
        executable_path: &str,
        socket_path: &str,
    ) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n    \
<key>Label</key>\n    \
<string>{job_label}</string>\n    \
<key>ProgramArguments</key>\n    \
<array>\n        \
<string>{executable_path}</string>\n        \
<string>--worker-socket</string>\n        \
<string>{socket_path}</string>\n    \
</array>\n    \
<key>MachServices</key>\n    \
<dict>\n        \
<key>{job_label}</key>\n        \
<true/>\n    \
</dict>\n\
</dict>\n\
</plist>\n"
        )
    }

    /// Generates code signing designated requirement string for an `SMJobBless` helper tool.
    ///
    /// # Arguments
    ///
    /// * `bundle_id` - Bundle identifier of the parent application or helper tool.
    /// * `team_id` - Optional Apple Developer Team ID (10 alphanumeric characters).
    ///
    /// # Returns
    ///
    /// Apple code signing requirement expression string.
    #[must_use]
    pub fn generate_smjobbless_requirement(bundle_id: &str, team_id: Option<&str>) -> String {
        team_id.map_or_else(
            || format!("identifier \"{bundle_id}\" and anchor apple generic"),
            |tid| {
                format!("identifier \"{bundle_id}\" and anchor apple generic and certificate leaf[subject.OU] = \"{tid}\"")
            },
        )
    }

    /// Determines the escalation method for a specific operating system and GUI environment.
    ///
    /// # Arguments
    ///
    /// * `os` - Target operating system name.
    /// * `has_gui` - Whether a graphical display server is active.
    ///
    /// # Returns
    ///
    /// Corresponding [`EscalationMethod`].
    #[must_use]
    pub fn detect_method_for(os: &str, has_gui: bool) -> EscalationMethod {
        match os {
            "windows" => EscalationMethod::ShellExecuteRunAs,
            "macos" if has_gui => EscalationMethod::OsascriptAdmin,
            "linux" if has_gui => EscalationMethod::PkExec,
            _ => EscalationMethod::Sudo,
        }
    }

    /// Constructs the complete command specification to spawn the privileged worker.
    ///
    /// # Arguments
    ///
    /// * `method` - Escalation method to apply.
    /// * `worker_binary` - Path to the installation worker binary.
    /// * `socket_path` - Path to the IPC domain socket or named pipe.
    ///
    /// # Returns
    ///
    /// A configured [`CommandSpec`].
    #[must_use]
    pub fn build_worker_command(
        method: EscalationMethod,
        worker_binary: &str,
        socket_path: &str,
    ) -> CommandSpec {
        let mut env = HashMap::new();
        env.insert("MSI_WORKER_SOCKET".to_string(), socket_path.to_string());

        match method {
            EscalationMethod::Direct => CommandSpec {
                program: worker_binary.to_string(),
                args: vec!["--worker-socket".to_string(), socket_path.to_string()],
                env,
            },
            EscalationMethod::Sudo => CommandSpec {
                program: "sudo".to_string(),
                args: vec![
                    "--preserve-env=MSI_WORKER_SOCKET".to_string(),
                    worker_binary.to_string(),
                    "--worker-socket".to_string(),
                    socket_path.to_string(),
                ],
                env,
            },
            EscalationMethod::PkExec => CommandSpec {
                program: "pkexec".to_string(),
                args: vec![
                    worker_binary.to_string(),
                    "--worker-socket".to_string(),
                    socket_path.to_string(),
                ],
                env,
            },
            EscalationMethod::SystemdRun => CommandSpec {
                program: "systemd-run".to_string(),
                args: vec![
                    "--wait".to_string(),
                    "--pipe".to_string(),
                    format!("--setenv=MSI_WORKER_SOCKET={socket_path}"),
                    worker_binary.to_string(),
                    "--worker-socket".to_string(),
                    socket_path.to_string(),
                ],
                env,
            },
            EscalationMethod::SMJobBless => CommandSpec {
                program: worker_binary.to_string(),
                args: vec![
                    "--auth-bless".to_string(),
                    "--worker-socket".to_string(),
                    socket_path.to_string(),
                ],
                env,
            },
            EscalationMethod::OsascriptAdmin => {
                let script = format!(
                    "do shell script \"MSI_WORKER_SOCKET='{socket_path}' '{worker_binary}' --worker-socket '{socket_path}'\" with administrator privileges"
                );
                CommandSpec {
                    program: "osascript".to_string(),
                    args: vec!["-e".to_string(), script],
                    env,
                }
            }
            EscalationMethod::ShellExecuteRunAs => CommandSpec {
                program: "cmd.exe".to_string(),
                args: vec![
                    "/c".to_string(),
                    format!("start powershell -Command Start-Process '{worker_binary}' -ArgumentList '--worker-socket {socket_path}' -Verb RunAs"),
                ],
                env,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that `detect_best_method` returns a valid escalation strategy.
    #[test]
    fn test_detect_best_method_returns_valid() {
        let method = PrivilegeEscalator::detect_best_method();
        assert!(matches!(
            method,
            EscalationMethod::Direct
                | EscalationMethod::Sudo
                | EscalationMethod::PkExec
                | EscalationMethod::SystemdRun
                | EscalationMethod::SMJobBless
                | EscalationMethod::OsascriptAdmin
                | EscalationMethod::ShellExecuteRunAs
        ));
    }

    /// Verifies detection logic across explicit OS and display configurations.
    #[test]
    fn test_detect_method_for_matrix() {
        assert_eq!(
            PrivilegeEscalator::detect_method_for("windows", false),
            EscalationMethod::ShellExecuteRunAs
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("windows", true),
            EscalationMethod::ShellExecuteRunAs
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("macos", false),
            EscalationMethod::Sudo
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("macos", true),
            EscalationMethod::OsascriptAdmin
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("linux", true),
            EscalationMethod::PkExec
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("linux", false),
            EscalationMethod::Sudo
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("freebsd", false),
            EscalationMethod::Sudo
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("freebsd", true),
            EscalationMethod::Sudo
        );
    }

    /// Tests macOS escalation selection helper for blessed, GUI, and headless contexts.
    #[test]
    fn test_detect_macos_method() {
        assert_eq!(
            PrivilegeEscalator::detect_macos_method(true, true),
            EscalationMethod::SMJobBless
        );
        assert_eq!(
            PrivilegeEscalator::detect_macos_method(false, true),
            EscalationMethod::SMJobBless
        );
        assert_eq!(
            PrivilegeEscalator::detect_macos_method(true, false),
            EscalationMethod::OsascriptAdmin
        );
        assert_eq!(
            PrivilegeEscalator::detect_macos_method(false, false),
            EscalationMethod::Sudo
        );
    }

    /// Tests `SMJobBless` launchd plist generation.
    #[test]
    fn test_generate_smjobbless_plist() {
        let plist = PrivilegeEscalator::generate_smjobbless_plist(
            "com.example.msi.helper",
            "/Library/PrivilegedHelperTools/com.example.msi.helper",
            "/tmp/msi-worker.sock",
        );
        assert!(plist.contains("<key>Label</key>"));
        assert!(plist.contains("<string>com.example.msi.helper</string>"));
        assert!(plist
            .contains("<string>/Library/PrivilegedHelperTools/com.example.msi.helper</string>"));
        assert!(plist.contains("<string>--worker-socket</string>"));
        assert!(plist.contains("<string>/tmp/msi-worker.sock</string>"));
    }

    /// Tests `SMJobBless` code signing designated requirement generation.
    #[test]
    fn test_generate_smjobbless_requirement() {
        let req_with_team = PrivilegeEscalator::generate_smjobbless_requirement(
            "com.example.msi.helper",
            Some("ABCDE12345"),
        );
        assert!(req_with_team.contains("identifier \"com.example.msi.helper\""));
        assert!(req_with_team.contains("certificate leaf[subject.OU] = \"ABCDE12345\""));

        let req_no_team =
            PrivilegeEscalator::generate_smjobbless_requirement("com.example.msi.helper", None);
        assert!(req_no_team.contains("identifier \"com.example.msi.helper\""));
        assert!(!req_no_team.contains("certificate leaf[subject.OU]"));
    }

    /// Tests detection of best escalation method across macOS terminal vs GUI contexts.
    #[test]
    #[cfg(target_os = "macos")]
    fn test_detect_best_method_macos_contexts() {
        // 1. Both unset -> GUI osascript
        std::env::remove_var("TERM_PROGRAM");
        std::env::remove_var("SSH_CONNECTION");
        assert_eq!(
            PrivilegeEscalator::detect_best_method(),
            EscalationMethod::OsascriptAdmin
        );

        // 2. TERM_PROGRAM set -> Terminal sudo
        std::env::set_var("TERM_PROGRAM", "Terminal");
        assert_eq!(
            PrivilegeEscalator::detect_best_method(),
            EscalationMethod::Sudo
        );

        // 3. SSH_CONNECTION set, TERM_PROGRAM unset -> SSH session sudo
        std::env::remove_var("TERM_PROGRAM");
        std::env::set_var("SSH_CONNECTION", "10.0.0.1 50000 10.0.0.2 22");
        assert_eq!(
            PrivilegeEscalator::detect_best_method(),
            EscalationMethod::Sudo
        );

        // Clean up
        std::env::remove_var("SSH_CONNECTION");
    }

    /// Tests worker command construction for all supported escalation methods.
    #[test]
    fn test_build_worker_command_all_methods() {
        let methods = [
            EscalationMethod::Direct,
            EscalationMethod::Sudo,
            EscalationMethod::PkExec,
            EscalationMethod::SystemdRun,
            EscalationMethod::SMJobBless,
            EscalationMethod::OsascriptAdmin,
            EscalationMethod::ShellExecuteRunAs,
        ];

        for m in methods {
            let spec = PrivilegeEscalator::build_worker_command(
                m,
                "/usr/local/bin/msi-worker",
                "/tmp/msi-worker.sock",
            );
            assert_ne!(spec.program, "");
            assert_ne!(spec.args.len(), 0);
            assert!(spec.env.contains_key("MSI_WORKER_SOCKET"));
            assert_eq!(
                spec.env.get("MSI_WORKER_SOCKET"),
                Some(&"/tmp/msi-worker.sock".to_string())
            );
            if m == EscalationMethod::OsascriptAdmin {
                assert_eq!(spec.program, "osascript");
                assert_eq!(spec.args[0], "-e");
                assert!(spec.args[1].contains("with administrator privileges"));
            }
        }
    }
}

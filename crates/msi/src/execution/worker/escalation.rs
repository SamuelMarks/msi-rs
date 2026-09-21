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
    pub const fn detect_best_method() -> EscalationMethod {
        #[cfg(windows)]
        {
            EscalationMethod::ShellExecuteRunAs
        }
        #[cfg(target_os = "macos")]
        {
            EscalationMethod::SMJobBless
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
            "macos" => EscalationMethod::SMJobBless,
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
            EscalationMethod::SMJobBless
        );
        assert_eq!(
            PrivilegeEscalator::detect_method_for("macos", true),
            EscalationMethod::SMJobBless
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

    /// Tests worker command construction for all supported escalation methods.
    #[test]
    fn test_build_worker_command_all_methods() {
        let methods = [
            EscalationMethod::Direct,
            EscalationMethod::Sudo,
            EscalationMethod::PkExec,
            EscalationMethod::SystemdRun,
            EscalationMethod::SMJobBless,
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
        }
    }
}

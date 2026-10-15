//! CLI command-line parsing, shell translation, PATH lookup, and TCP port probing helpers.

use std::path::{Path, PathBuf};

/// Probes whether a TCP port is available for local socket binding.
///
/// Returns `true` if a listener can successfully bind to `127.0.0.1:port`
/// (meaning the port is free), and `false` if the port is already occupied.
///
/// # Arguments
///
/// * `port` - TCP port number to probe (1-65535).
///
/// # Returns
///
/// `true` if port is available, `false` otherwise.
#[must_use]
pub fn probe_port_available(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Translates a Windows shell command line to POSIX syntax if executing on non-Windows platforms.
///
/// Converts invocations like `cmd.exe /c [INSTALLFOLDER]libscript\libscript.cmd ...` into
/// `/bin/sh [INSTALLFOLDER]/libscript/libscript.sh ...` or dispatches to the corresponding script.
/// On Windows, preserves native execution semantics.
///
/// # Arguments
///
/// * `prog` - Original executable program path.
/// * `args` - Original command arguments.
///
/// # Returns
///
/// Tuple of `(translated_program_path, translated_argument_vector)`.
#[must_use]
pub fn translate_shell_command(prog: &Path, args: &[String]) -> (PathBuf, Vec<String>) {
    #[cfg(not(target_os = "windows"))]
    let prog_str = prog.to_string_lossy();
    #[cfg(not(target_os = "windows"))]
    let is_cmd_exe = prog_str.eq_ignore_ascii_case("cmd.exe")
        || prog_str.eq_ignore_ascii_case("cmd")
        || prog_str.ends_with(r"\cmd.exe")
        || prog_str.ends_with("/cmd.exe");

    #[cfg(not(target_os = "windows"))]
    let is_batch = prog
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
    #[cfg(not(target_os = "windows"))]
    if is_batch {
        let mut script_path_buf = prog.to_string_lossy().replace('\\', "/");
        script_path_buf.truncate(script_path_buf.len() - 4);
        script_path_buf.push_str(".sh");
        let mut sh_args = vec![script_path_buf];
        for a in args {
            sh_args.push(a.replace('\\', "/"));
        }
        return (PathBuf::from("/bin/sh"), sh_args);
    }

    #[cfg(not(target_os = "windows"))]
    if is_cmd_exe {
        let mut new_args = Vec::new();
        let mut skip_next = false;
        let mut script_cmd_found = None;

        for (idx, arg) in args.iter().enumerate() {
            if skip_next {
                skip_next = false;
                continue;
            }
            if arg.eq_ignore_ascii_case("/c") || arg.eq_ignore_ascii_case("/k") {
                // If there's an immediate next argument, that is the target script or command
                if let Some(target_cmd) = args.get(idx + 1) {
                    script_cmd_found = Some(target_cmd.clone());
                    skip_next = true;
                }
            } else if arg.starts_with("/c") || arg.starts_with("/C") {
                script_cmd_found = Some(arg[2..].trim().to_string());
            } else if script_cmd_found.is_none() {
                script_cmd_found = Some(arg.clone());
            } else {
                new_args.push(arg.clone());
            }
        }

        if let Some(cmd_line) = script_cmd_found {
            // Translate backslashes to forward slashes and `.cmd`/`.bat` to `.sh`
            let normalized = cmd_line.replace('\\', "/");
            let (script_path_str, mut script_args) = parse_command_line(&normalized);
            let mut script_path_buf = script_path_str.to_string_lossy().to_string();
            let is_script_ext = Path::new(&script_path_buf).extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat")
            });
            if is_script_ext {
                script_path_buf.truncate(script_path_buf.len() - 4);
                script_path_buf.push_str(".sh");
            }
            for arg in &mut new_args {
                *arg = arg.replace('\\', "/");
            }
            script_args.append(&mut new_args);

            // Execute via /bin/sh
            let mut sh_args = vec![script_path_buf];
            sh_args.append(&mut script_args);
            return (PathBuf::from("/bin/sh"), sh_args);
        }

        return (PathBuf::from("/bin/sh"), Vec::new());
    }

    (prog.to_path_buf(), args.to_vec())
}

/// Splits a command line string into an executable path and argument list, respecting quotes.
///
/// # Arguments
///
/// * `cmd` - Command line string to parse.
///
/// # Returns
///
/// Tuple of `(program_path, argument_vector)`.
#[must_use]
pub fn parse_command_line(cmd: &str) -> (PathBuf, Vec<String>) {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in cmd.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' => {
                if in_quotes {
                    current.push(ch);
                } else if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            c => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        (PathBuf::new(), Vec::new())
    } else {
        let prog = PathBuf::from(parts.remove(0));
        (prog, parts)
    }
}

/// Checks whether an executable exists in the system `PATH`.
///
/// # Arguments
///
/// * `prog` - The prog argument.
///
/// # Returns
///
/// The return value.
pub(crate) fn is_executable_in_path(prog: &Path) -> bool {
    if prog.is_absolute() {
        return prog.exists();
    }
    std::env::var_os("PATH").is_some_and(|path_var| {
        std::env::split_paths(&path_var).any(|dir| dir.join(prog).is_file())
    })
}

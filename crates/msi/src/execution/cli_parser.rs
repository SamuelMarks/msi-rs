//! Command-line interface argument parser providing complete parity with `msiexec.exe`.
//!
//! Grounded directly in official Microsoft Windows Installer Command-Line Options:
//! - Action modes: `/i`, `/x`, `/a`, `/f` (with repair flags), `/j` (advertise), `/p` (patch).
//! - UI levels: `/qn`, `/qb`, `/qb!`, `/qr`, `/qf`.
//! - Logging options: `/l` (flags `i,w,e,a,r,u,c,m,o,p,v,x,+,!`).
//! - Public property overrides: `PROPERTY=Value`.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Repair mode flags corresponding to `/f[p|o|e|d|c|a|u|m|s|v]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct RepairFlags {
    /// `p`: Reinstall only if file is missing.
    pub reinstall_missing: bool,
    /// `o`: Reinstall if file is missing or older version.
    pub reinstall_older: bool,
    /// `e`: Reinstall if file is missing or equal/older version.
    pub reinstall_equal_or_older: bool,
    /// `d`: Reinstall if file is missing or different version.
    pub reinstall_different: bool,
    /// `c`: Reinstall if file is missing or checksum differs.
    pub reinstall_checksum: bool,
    /// `a`: Reinstall all files regardless of checksum/version.
    pub reinstall_all: bool,
    /// `u`: Rewrite all required user registry entries.
    pub rewrite_user_registry: bool,
    /// `m`: Rewrite all required machine registry entries.
    pub rewrite_machine_registry: bool,
    /// `s`: Overwrite all existing shortcuts.
    pub overwrite_shortcuts: bool,
    /// `v`: Run from source and re-cache local package.
    pub recache_source: bool,
}

impl RepairFlags {
    /// Parses a string of repair flag characters.
    ///
    /// # Arguments
    ///
    /// * `flags` - Character sequence (e.g. `"omus"`).
    ///
    /// # Returns
    ///
    /// Parsed [`RepairFlags`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if an unrecognized flag character is encountered.
    pub fn parse(flags: &str) -> Result<Self> {
        let mut res = Self::default();
        for ch in flags.chars() {
            match ch.to_ascii_lowercase() {
                'p' => res.reinstall_missing = true,
                'o' => res.reinstall_older = true,
                'e' => res.reinstall_equal_or_older = true,
                'd' => res.reinstall_different = true,
                'c' => res.reinstall_checksum = true,
                'a' => res.reinstall_all = true,
                'u' => res.rewrite_user_registry = true,
                'm' => res.rewrite_machine_registry = true,
                's' => res.overwrite_shortcuts = true,
                'v' => res.recache_source = true,
                other => {
                    return Err(Error::InvalidArgument {
                        argument: format!("/f{other}"),
                        reason: format!("unknown repair flag '{other}'"),
                    });
                }
            }
        }
        Ok(res)
    }
}

/// Target scope for advertisement (`/j[u|m]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvertiseScope {
    /// `/ju`: Advertise to current user.
    User,
    /// `/jm`: Advertise to all users on this machine.
    Machine,
}

/// Primary execution action mode requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMode {
    /// `/i <package>`: Install product.
    Install {
        /// Path to the package file.
        package_path: String,
    },
    /// `/x <package>`: Uninstall product.
    Uninstall {
        /// Path or product code of package to uninstall.
        package_path: String,
    },
    /// `/a <package>`: Administrative installation.
    Administrative {
        /// Path to package file.
        package_path: String,
    },
    /// `/f <flags> <package>`: Repair product.
    Repair {
        /// Specific repair flags.
        flags: RepairFlags,
        /// Path to package file.
        package_path: String,
    },
    /// `/j <scope> <package>`: Advertise product.
    Advertise {
        /// Target scope (user or machine).
        scope: AdvertiseScope,
        /// Path to package file.
        package_path: String,
    },
    /// `/p <patch>`: Apply patch package (`.msp`).
    ApplyPatch {
        /// Path to patch file.
        patch_path: String,
    },
}

/// UI display level requested via `/q` options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiLevel {
    /// `/qf`: Full UI (all wizards and dialogs).
    #[default]
    Full,
    /// `/qr`: Reduced UI.
    Reduced,
    /// `/qb`: Basic UI (progress bar only).
    Basic {
        /// Whether Cancel button is disabled (`/qb!`).
        no_cancel: bool,
    },
    /// `/qn`: Quiet / Silent (No UI).
    None,
}

/// Logging mode flags corresponding to `/l[i|w|e|a|r|u|c|m|o|p|v|x|+|!]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct LoggingOptions {
    /// Log file destination path.
    pub log_file: String,
    /// `i`: Status messages.
    pub status: bool,
    /// `w`: Non-fatal warnings.
    pub warnings: bool,
    /// `e`: All error messages.
    pub errors: bool,
    /// `a`: Action starts.
    pub action_starts: bool,
    /// `r`: Action-specific records.
    pub action_records: bool,
    /// `u`: User requests.
    pub user_requests: bool,
    /// `c`: Initial UI parameters.
    pub ui_parameters: bool,
    /// `m`: Out of memory / fatal exit.
    pub out_of_memory: bool,
    /// `o`: Out of disk space messages.
    pub out_of_disk: bool,
    /// `p`: Terminal properties.
    pub terminal_props: bool,
    /// `v`: Verbose output.
    pub verbose: bool,
    /// `x`: Extra debugging information.
    pub extra_debugging: bool,
    /// `+`: Append to existing log.
    pub append: bool,
    /// `!`: Flush each line immediately to disk.
    pub flush_immediately: bool,
}

impl LoggingOptions {
    /// Parses logging flags and log file path from argument pair.
    ///
    /// # Arguments
    ///
    /// * `flags` - Flag characters following `/l` (e.g. `"iwearm+!"` or `"*v"`).
    /// * `log_file` - Destination file path.
    ///
    /// # Returns
    ///
    /// Parsed [`LoggingOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if an unrecognized logging flag is encountered.
    pub fn parse<F: AsRef<str>, S: Into<String>>(flags: F, log_file: S) -> Result<Self> {
        Self::parse_impl(flags.as_ref(), log_file.into())
    }

    /// Internal non-generic logging options parser.
    fn parse_impl(flags: &str, log_file: String) -> Result<Self> {
        let mut opts = Self {
            log_file,
            ..Self::default()
        };

        for ch in flags.chars() {
            match ch {
                'i' => opts.status = true,
                'w' => opts.warnings = true,
                'e' => opts.errors = true,
                'a' => opts.action_starts = true,
                'r' => opts.action_records = true,
                'u' => opts.user_requests = true,
                'c' => opts.ui_parameters = true,
                'm' => opts.out_of_memory = true,
                'o' => opts.out_of_disk = true,
                'p' => opts.terminal_props = true,
                'v' => opts.verbose = true,
                'x' => opts.extra_debugging = true,
                '+' => opts.append = true,
                '!' => opts.flush_immediately = true,
                '*' => {
                    // Wildcard: all status flags
                    opts.status = true;
                    opts.warnings = true;
                    opts.errors = true;
                    opts.action_starts = true;
                    opts.action_records = true;
                    opts.user_requests = true;
                    opts.ui_parameters = true;
                    opts.out_of_memory = true;
                    opts.out_of_disk = true;
                    opts.terminal_props = true;
                }
                other => {
                    return Err(Error::InvalidArgument {
                        argument: format!("/l{other}"),
                        reason: format!("unknown logging flag '{other}'"),
                    });
                }
            }
        }

        Ok(opts)
    }
}

/// Fully parsed command-line configuration for `msiexec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiExecOptions {
    /// Action mode to execute.
    pub action: ActionMode,
    /// User interface display level.
    pub ui_level: UiLevel,
    /// Optional logging configuration.
    pub logging: Option<LoggingOptions>,
    /// Public property overrides (`PROPERTY=Value`).
    pub properties: HashMap<String, String>,
}

impl MsiExecOptions {
    /// Parses command-line arguments matching `msiexec.exe` syntax.
    ///
    /// # Arguments
    ///
    /// * `args` - Iterator of command line strings (excluding executable name).
    ///
    /// # Returns
    ///
    /// Parsed [`MsiExecOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if options are missing, conflicting, or malformed.
    pub fn parse<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let arg_list: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        Self::parse_strings(&arg_list)
    }

    /// Internal parser operating on concrete string slice.
    #[allow(clippy::too_many_lines)]
    fn parse_strings(arg_list: &[String]) -> Result<Self> {
        if arg_list.is_empty() {
            return Err(Error::InvalidArgument {
                argument: "args".to_string(),
                reason: "no arguments provided to msiexec".to_string(),
            });
        }

        let mut action: Option<ActionMode> = None;
        let mut ui_level = UiLevel::Full;
        let mut logging: Option<LoggingOptions> = None;
        let mut properties: HashMap<String, String> = HashMap::new();

        let mut i = 0;
        while i < arg_list.len() {
            let arg = &arg_list[i];

            if arg.starts_with('/') || arg.starts_with('-') {
                let opt = &arg[1..];
                let opt_lower = opt.to_ascii_lowercase();

                if opt_lower.starts_with('i') {
                    // /i <package>
                    let path = Self::extract_param(opt, "i", arg_list, &mut i)?;
                    action = Some(ActionMode::Install { package_path: path });
                } else if opt_lower.starts_with('x') {
                    // /x <package>
                    let path = Self::extract_param(opt, "x", arg_list, &mut i)?;
                    action = Some(ActionMode::Uninstall { package_path: path });
                } else if opt_lower.starts_with('a') {
                    // /a <package>
                    let path = Self::extract_param(opt, "a", arg_list, &mut i)?;
                    action = Some(ActionMode::Administrative { package_path: path });
                } else if opt_lower.starts_with('f') {
                    // /f[flags] <package>
                    let flag_chars = &opt[1..];
                    let flags = if flag_chars.is_empty() {
                        RepairFlags {
                            reinstall_older: true,
                            ..RepairFlags::default()
                        }
                    } else {
                        RepairFlags::parse(flag_chars)?
                    };
                    i += 1;
                    if i >= arg_list.len() {
                        return Err(Error::InvalidArgument {
                            argument: arg.clone(),
                            reason: "missing package path for repair".to_string(),
                        });
                    }
                    action = Some(ActionMode::Repair {
                        flags,
                        package_path: arg_list[i].clone(),
                    });
                } else if opt_lower.starts_with('j') {
                    // /j[u|m] <package>
                    let scope = match opt_lower.chars().nth(1) {
                        Some('u') => AdvertiseScope::User,
                        Some('m') | None => AdvertiseScope::Machine,
                        Some(other) => {
                            return Err(Error::InvalidArgument {
                                argument: arg.clone(),
                                reason: format!("unknown advertise scope '{other}'"),
                            });
                        }
                    };
                    i += 1;
                    if i >= arg_list.len() {
                        return Err(Error::InvalidArgument {
                            argument: arg.clone(),
                            reason: "missing package path for advertise".to_string(),
                        });
                    }
                    action = Some(ActionMode::Advertise {
                        scope,
                        package_path: arg_list[i].clone(),
                    });
                } else if opt_lower.starts_with('p') {
                    // /p <patch>
                    let path = Self::extract_param(opt, "p", arg_list, &mut i)?;
                    action = Some(ActionMode::ApplyPatch { patch_path: path });
                } else if opt_lower.starts_with("qn") {
                    ui_level = UiLevel::None;
                } else if opt_lower.starts_with("qb!") {
                    ui_level = UiLevel::Basic { no_cancel: true };
                } else if opt_lower.starts_with("qb") {
                    ui_level = UiLevel::Basic { no_cancel: false };
                } else if opt_lower.starts_with("qr") {
                    ui_level = UiLevel::Reduced;
                } else if opt_lower.starts_with("qf") || opt_lower == "q" {
                    ui_level = UiLevel::Full;
                } else if opt_lower.starts_with('l') {
                    // /l[flags] <logfile>
                    let flag_chars = &opt[1..];
                    i += 1;
                    if i >= arg_list.len() {
                        return Err(Error::InvalidArgument {
                            argument: arg.clone(),
                            reason: "missing log file path".to_string(),
                        });
                    }
                    logging = Some(LoggingOptions::parse(flag_chars, &arg_list[i])?);
                } else {
                    return Err(Error::InvalidArgument {
                        argument: arg.clone(),
                        reason: format!("unrecognized command-line option '{arg}'"),
                    });
                }
            } else if let Some((key, val)) = arg.split_once('=') {
                // Public property override: PROPERTY=Value
                properties.insert(key.to_string(), val.to_string());
            } else {
                return Err(Error::InvalidArgument {
                    argument: arg.clone(),
                    reason: format!("unexpected argument '{arg}'"),
                });
            }

            i += 1;
        }

        let Some(act) = action else {
            return Err(Error::InvalidArgument {
                argument: "action".to_string(),
                reason: "no action mode specified (/i, /x, /a, /f, /j, /p)".to_string(),
            });
        };

        Ok(Self {
            action: act,
            ui_level,
            logging,
            properties,
        })
    }

    /// Helper to extract option parameter either attached or in the next argument.
    fn extract_param(
        opt: &str,
        prefix: &str,
        arg_list: &[String],
        i: &mut usize,
    ) -> Result<String> {
        let rest = &opt[prefix.len()..];
        if rest.is_empty() {
            *i += 1;
            if *i >= arg_list.len() {
                return Err(Error::InvalidArgument {
                    argument: format!("/{prefix}"),
                    reason: format!("missing argument following /{prefix}"),
                });
            }
            Ok(arg_list[*i].clone())
        } else {
            Ok(rest.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_install() -> Result<()> {
        let opts =
            MsiExecOptions::parse(["/i", "setup.msi", "TRANSFORMS=custom.mst", "ADDLOCAL=ALL"])?;
        assert_eq!(
            opts.action,
            ActionMode::Install {
                package_path: "setup.msi".to_string(),
            }
        );
        assert_eq!(opts.ui_level, UiLevel::Full);
        assert_eq!(
            opts.properties.get("TRANSFORMS"),
            Some(&"custom.mst".to_string())
        );
        assert_eq!(opts.properties.get("ADDLOCAL"), Some(&"ALL".to_string()));
        Ok(())
    }

    #[test]
    fn test_parse_uninstall_and_quiet() -> Result<()> {
        let opts = MsiExecOptions::parse(["/x", "package.msi", "/qn"])?;
        assert_eq!(
            opts.action,
            ActionMode::Uninstall {
                package_path: "package.msi".to_string(),
            }
        );
        assert_eq!(opts.ui_level, UiLevel::None);
        Ok(())
    }

    #[test]
    fn test_parse_repair_flags() -> Result<()> {
        let opts = MsiExecOptions::parse(["/fomus", "app.msi", "/qb!"])?;
        let expected_flags = RepairFlags {
            reinstall_older: true,
            rewrite_user_registry: true,
            rewrite_machine_registry: true,
            overwrite_shortcuts: true,
            ..RepairFlags::default()
        };
        assert_eq!(
            opts.action,
            ActionMode::Repair {
                flags: expected_flags,
                package_path: "app.msi".to_string(),
            }
        );
        assert_eq!(opts.ui_level, UiLevel::Basic { no_cancel: true });
        Ok(())
    }

    #[test]
    fn test_parse_admin_and_advertise() -> Result<()> {
        let admin_opts = MsiExecOptions::parse(["/a", "admin.msi"])?;
        assert_eq!(
            admin_opts.action,
            ActionMode::Administrative {
                package_path: "admin.msi".to_string(),
            }
        );

        let adv_opts = MsiExecOptions::parse(["/ju", "user.msi"])?;
        assert_eq!(
            adv_opts.action,
            ActionMode::Advertise {
                scope: AdvertiseScope::User,
                package_path: "user.msi".to_string(),
            }
        );

        let patch_opts = MsiExecOptions::parse(["/p", "update.msp"])?;
        assert_eq!(
            patch_opts.action,
            ActionMode::ApplyPatch {
                patch_path: "update.msp".to_string(),
            }
        );
        Ok(())
    }

    #[test]
    fn test_parse_logging() -> Result<()> {
        let opts = MsiExecOptions::parse(["/i", "pkg.msi", "/lv*+!", "install.log"])?;
        let expected_log = LoggingOptions {
            log_file: "install.log".to_string(),
            status: true,
            warnings: true,
            errors: true,
            action_starts: true,
            action_records: true,
            user_requests: true,
            ui_parameters: true,
            out_of_memory: true,
            out_of_disk: true,
            terminal_props: true,
            verbose: true,
            extra_debugging: false,
            append: true,
            flush_immediately: true,
        };
        assert_eq!(opts.logging, Some(expected_log));

        // Test all individual logging flags
        let all_log = LoggingOptions::parse("iwearucmopvx", "all.log")?;
        assert!(all_log.status);
        assert!(all_log.warnings);
        assert!(all_log.errors);
        assert!(all_log.action_starts);
        assert!(all_log.action_records);
        assert!(all_log.user_requests);
        assert!(all_log.ui_parameters);
        assert!(all_log.out_of_memory);
        assert!(all_log.out_of_disk);
        assert!(all_log.terminal_props);
        assert!(all_log.verbose);
        assert!(all_log.extra_debugging);

        // Uppercase /L flag
        let opts_upper_l = MsiExecOptions::parse(["/I", "pkg.msi", "/L*", "out.log"])?;
        assert!(opts_upper_l.logging.is_some());

        Ok(())
    }

    #[test]
    fn test_parse_attached_params() -> Result<()> {
        let parsed_i = MsiExecOptions::parse(["-ipackage.msi", "/qr"])?;
        assert_eq!(
            parsed_i.action,
            ActionMode::Install {
                package_path: "package.msi".to_string(),
            }
        );
        assert_eq!(parsed_i.ui_level, UiLevel::Reduced);

        let parsed_x = MsiExecOptions::parse(["-xpackage.msi", "/qb"])?;
        assert_eq!(
            parsed_x.action,
            ActionMode::Uninstall {
                package_path: "package.msi".to_string(),
            }
        );
        assert_eq!(parsed_x.ui_level, UiLevel::Basic { no_cancel: false });

        let parsed_a = MsiExecOptions::parse(["-apackage.msi", "/qf"])?;
        assert_eq!(
            parsed_a.action,
            ActionMode::Administrative {
                package_path: "package.msi".to_string(),
            }
        );
        assert_eq!(parsed_a.ui_level, UiLevel::Full);

        let parsed_p = MsiExecOptions::parse(["-ppatch.msp", "/q"])?;
        assert_eq!(
            parsed_p.action,
            ActionMode::ApplyPatch {
                patch_path: "patch.msp".to_string(),
            }
        );
        assert_eq!(parsed_p.ui_level, UiLevel::Full);

        Ok(())
    }

    #[test]
    fn test_parse_uppercase_and_repair() -> Result<()> {
        // Uppercase UI flags
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QR"])?.ui_level,
            UiLevel::Reduced
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QN"])?.ui_level,
            UiLevel::None
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QB!"])?.ui_level,
            UiLevel::Basic { no_cancel: true }
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QB"])?.ui_level,
            UiLevel::Basic { no_cancel: false }
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QF"])?.ui_level,
            UiLevel::Full
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/Q"])?.ui_level,
            UiLevel::Full
        );

        // Uppercase action switches
        assert!(matches!(
            MsiExecOptions::parse(["/X", "package.msi"])?.action,
            ActionMode::Uninstall { .. }
        ));
        assert!(matches!(
            MsiExecOptions::parse(["/A", "package.msi"])?.action,
            ActionMode::Administrative { .. }
        ));
        assert!(matches!(
            MsiExecOptions::parse(["/P", "patch.msp"])?.action,
            ActionMode::ApplyPatch { .. }
        ));

        // Advertise scopes
        assert_eq!(
            MsiExecOptions::parse(["/JU", "adv.msi"])?.action,
            ActionMode::Advertise {
                scope: AdvertiseScope::User,
                package_path: "adv.msi".to_string(),
            }
        );
        assert_eq!(
            MsiExecOptions::parse(["/j", "adv.msi"])?.action,
            ActionMode::Advertise {
                scope: AdvertiseScope::Machine,
                package_path: "adv.msi".to_string(),
            }
        );
        assert_eq!(
            MsiExecOptions::parse(["/jm", "adv.msi"])?.action,
            ActionMode::Advertise {
                scope: AdvertiseScope::Machine,
                package_path: "adv.msi".to_string(),
            }
        );

        // All repair flags
        let all_flags = RepairFlags::parse("poedcaumsv")?;
        assert!(all_flags.reinstall_missing);
        assert!(all_flags.reinstall_older);
        assert!(all_flags.reinstall_equal_or_older);
        assert!(all_flags.reinstall_different);
        assert!(all_flags.reinstall_checksum);
        assert!(all_flags.reinstall_all);
        assert!(all_flags.rewrite_user_registry);
        assert!(all_flags.rewrite_machine_registry);
        assert!(all_flags.overwrite_shortcuts);
        assert!(all_flags.recache_source);

        // Default repair option with no flags attached
        let parsed_repair = MsiExecOptions::parse(["/f", "app.msi"])?;
        let expected_default_flags = RepairFlags {
            reinstall_older: true,
            ..RepairFlags::default()
        };
        assert_eq!(
            parsed_repair.action,
            ActionMode::Repair {
                flags: expected_default_flags,
                package_path: "app.msi".to_string(),
            }
        );

        // Uppercase /F
        assert!(matches!(
            MsiExecOptions::parse(["/FOMUS", "app.msi"])?.action,
            ActionMode::Repair { .. }
        ));

        Ok(())
    }

    #[test]
    fn test_parse_errors() {
        assert!(MsiExecOptions::parse::<[&str; 0], &str>([]).is_err());
        assert!(MsiExecOptions::parse(["/i"]).is_err());
        assert!(MsiExecOptions::parse(["/x"]).is_err());
        assert!(MsiExecOptions::parse(["/a"]).is_err());
        assert!(MsiExecOptions::parse(["/p"]).is_err());
        assert!(MsiExecOptions::parse(["/j"]).is_err());
        assert!(MsiExecOptions::parse(["/ju"]).is_err());
        assert!(MsiExecOptions::parse(["/jm"]).is_err());
        assert!(MsiExecOptions::parse(["/f"]).is_err());
        assert!(MsiExecOptions::parse(["/fomus"]).is_err());
        assert!(MsiExecOptions::parse(["/l"]).is_err());
        assert!(MsiExecOptions::parse(["/lv"]).is_err());
        assert!(MsiExecOptions::parse(["/unknown"]).is_err());
        assert!(MsiExecOptions::parse(["/fz", "pkg.msi"]).is_err());
        assert!(MsiExecOptions::parse(["/jx", "pkg.msi"]).is_err());
        assert!(MsiExecOptions::parse(["/l?", "log.txt"]).is_err());
        assert!(MsiExecOptions::parse(["invalid_arg_without_equals"]).is_err());
        assert!(MsiExecOptions::parse(["PROPERTY=Val"]).is_err()); // no action
    }
}

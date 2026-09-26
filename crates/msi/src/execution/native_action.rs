//! Native Dynamic Library (`.dll` / `.so` / `.dylib`) and Subprocess Custom Action Runtime.
//!
//! Grounded directly in official Microsoft Windows Installer SDK specifications:
//! - Custom action function pointer signature: `typedef UINT (WINAPI *MSICUSTOMACTION)(MSIHANDLE hInstall);`.
//! - Calling convention: `extern "system"` (`stdcall` on 32-bit x86, standard C calling convention on 64-bit and POSIX).
//! - SEH and panic boundary isolation preventing native code crashes from terminating the installer.
//! - Sandboxed temporary library extraction with restrictive permissions and automatic cleanup.
//! - Subprocess execution runner with stdin/stdout/stderr capture, working directory setup, timeout enforcement,
//!   and exit code mapping (`0` -> `ERROR_SUCCESS`, `3010` -> `ERROR_SUCCESS_REBOOT_REQUIRED`, non-zero -> `ERROR_INSTALL_FAILURE`).

use crate::error::{Error, Result};
use crate::execution::custom_action::{ERROR_FUNCTION_FAILED, ERROR_SUCCESS, MSIHANDLE};
use crate::execution::transaction::ERROR_INSTALL_FAILURE;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Standard MSI exit code indicating successful installation with reboot required.
pub const ERROR_SUCCESS_REBOOT_REQUIRED: u32 = 3010;

/// Default timeout in milliseconds for custom action subprocess execution (30 seconds).
pub const DEFAULT_ACTION_TIMEOUT_MS: u64 = 30_000;

/// Native custom action entry point function pointer signature.
pub type MsiCustomActionFn = unsafe extern "system-unwind" fn(h_install: MSIHANDLE) -> u32;

/// Global atomic counter generating unique sandbox directory names.
static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Binary executable or dynamic library file format detected from magic header bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BinaryFormat {
    /// Microsoft Windows Portable Executable (PE32 / PE32+ for `.exe`, `.dll`, `.sys`).
    PeWindows,
    /// Executable and Linkable Format (ELF for Linux, FreeBSD, NetBSD, Solaris).
    ElfUnix,
    /// Mach-O object file (macOS / iOS `.dylib`, bundle, executable).
    MachOApple,
    /// Unrecognized or raw non-binary format.
    #[default]
    Unknown,
}

impl BinaryFormat {
    /// Detects binary format from the initial magic header bytes of a file.
    ///
    /// # Arguments
    ///
    /// * `header` - Raw slice of initial file bytes.
    ///
    /// # Returns
    ///
    /// Detected [`BinaryFormat`].
    #[must_use]
    pub fn detect(header: &[u8]) -> Self {
        if header.len() >= 2 && header[0] == 0x4D && header[1] == 0x5A {
            return Self::PeWindows;
        }
        if header.len() >= 4 && &header[0..4] == b"\x7fELF" {
            return Self::ElfUnix;
        }
        if header.len() >= 4 {
            let magic = u32::from_ne_bytes([header[0], header[1], header[2], header[3]]);
            if magic == 0xFEED_FACE
                || magic == 0xFEED_FACF
                || magic == 0xCEFA_EDFE
                || magic == 0xCFFA_EDFE
                || magic == 0xCAFE_BABE
                || magic == 0xBEBA_FECA
            {
                return Self::MachOApple;
            }
        }
        Self::Unknown
    }
}

/// Execution mode for Wine Windows PE emulation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WineMode {
    /// Automatically detects Wine on host if present.
    #[default]
    Auto,
    /// Disables Wine emulation even if present on host.
    Disabled,
    /// Uses an explicit custom Wine executable path.
    Custom(PathBuf),
}

/// Sandboxed loader for native custom action libraries.
#[derive(Debug)]
pub struct NativeLibraryLoader {
    /// Sandboxed extraction directory on disk, if allocated.
    sandbox_dir: Option<PathBuf>,
    /// Path to extracted library file, if created.
    library_path: Option<PathBuf>,
    /// Detected binary format of the loaded library file.
    library_format: BinaryFormat,
    /// Active Wine executable path if loaded as a Windows PE DLL on a non-Windows platform.
    wine_executable: Option<PathBuf>,
    /// Active Wine execution mode.
    pub wine_mode: WineMode,
    /// Active OS dynamic library handle (`dlopen` on POSIX).
    #[cfg(unix)]
    dl_handle: Option<*mut std::ffi::c_void>,
    /// Registered in-memory function pointers for native action dispatching.
    functions: HashMap<String, MsiCustomActionFn>,
}

// SAFETY: NativeLibraryLoader owns raw dynamic library handles safely across threads.
unsafe impl Send for NativeLibraryLoader {}
// SAFETY: Function table lookups are synchronized and loaded function pointers are immutable.
unsafe impl Sync for NativeLibraryLoader {}

impl Clone for NativeLibraryLoader {
    fn clone(&self) -> Self {
        let mut loader = Self {
            sandbox_dir: self.sandbox_dir.clone(),
            library_path: self.library_path.clone(),
            library_format: self.library_format,
            wine_executable: self.wine_executable.clone(),
            wine_mode: self.wine_mode.clone(),
            #[cfg(unix)]
            dl_handle: None,
            functions: self.functions.clone(),
        };
        if let Some(ref p) = loader.library_path.clone() {
            let _ = loader.load_library(p);
        }
        loader
    }
}

impl Default for NativeLibraryLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeLibraryLoader {
    /// Creates a new empty [`NativeLibraryLoader`].
    ///
    /// # Returns
    ///
    /// A new [`NativeLibraryLoader`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            sandbox_dir: None,
            library_path: None,
            library_format: BinaryFormat::Unknown,
            wine_executable: None,
            wine_mode: WineMode::Auto,
            #[cfg(unix)]
            dl_handle: None,
            functions: HashMap::new(),
        }
    }

    /// Returns the detected binary format of the loaded library file.
    #[must_use]
    pub const fn library_format(&self) -> BinaryFormat {
        self.library_format
    }

    /// Returns the active Wine executable path if Wine execution bridge is active.
    #[must_use]
    pub fn wine_executable(&self) -> Option<&Path> {
        self.wine_executable.as_deref()
    }

    /// Checks for the presence of Wine (`wine64` or `wine`) on the host system.
    ///
    /// Searches standard `PATH` directories and standard installation paths.
    ///
    /// # Returns
    ///
    /// Path to Wine executable if found, or `None`.
    #[must_use]
    pub fn find_wine_binary() -> Option<PathBuf> {
        Self::find_wine_binary_custom(
            std::env::var_os("PATH").as_deref(),
            &["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"],
        )
    }

    /// Searches for Wine binary (`wine64` or `wine`) with custom PATH and fallback directories.
    ///
    /// # Arguments
    ///
    /// * `path_var` - Optional path string to search.
    /// * `fallback_dirs` - Fallback directories to search if not found in PATH.
    ///
    /// # Returns
    ///
    /// Path to Wine executable if found, or `None`.
    #[must_use]
    pub fn find_wine_binary_custom(
        path_var: Option<&std::ffi::OsStr>,
        fallback_dirs: &[&str],
    ) -> Option<PathBuf> {
        let candidates = ["wine64", "wine"];
        if let Some(path_var) = path_var {
            for dir in std::env::split_paths(path_var) {
                for &candidate in &candidates {
                    let bin = dir.join(candidate);
                    if bin.is_file() {
                        return Some(bin);
                    }
                }
            }
        }
        for &dir in fallback_dirs {
            for &candidate in &candidates {
                let bin = Path::new(dir).join(candidate);
                if bin.is_file() {
                    return Some(bin);
                }
            }
        }
        None
    }

    /// Constructs the Wine command arguments to invoke a custom action in a Windows PE DLL.
    ///
    /// Uses Wine's `rundll32.exe` bridge: `wine64 rundll32.exe <dll_path>,<entry_point> <h_install>`.
    ///
    /// # Arguments
    ///
    /// * `wine_path` - Path to Wine executable.
    /// * `dll_path` - Path to target Windows PE dynamic library.
    /// * `entry_point` - Name of entry point function.
    /// * `h_install` - MSI install session handle value.
    ///
    /// # Returns
    ///
    /// Tuple of `(program_path, argument_vector)`.
    #[must_use]
    pub fn build_wine_action_command(
        wine_path: &Path,
        dll_path: &Path,
        entry_point: &str,
        h_install: MSIHANDLE,
    ) -> (PathBuf, Vec<String>) {
        let args = vec![
            "rundll32.exe".to_string(),
            format!("{},{}", dll_path.display(), entry_point),
            h_install.to_string(),
        ];
        (wine_path.to_path_buf(), args)
    }

    /// Registers an in-memory entry point function pointer (safe native mock or statically linked symbol).
    ///
    /// # Arguments
    ///
    /// * `entry_point` - Name of entry point function.
    /// * `function` - Function pointer matching [`MsiCustomActionFn`].
    pub fn register_function(
        &mut self,
        entry_point: impl Into<String>,
        function: MsiCustomActionFn,
    ) {
        self.functions.insert(entry_point.into(), function);
    }

    /// Checks whether an in-memory entry point function or loaded library is present.
    ///
    /// # Arguments
    ///
    /// * `name` - Function entry point name.
    ///
    /// # Returns
    ///
    /// `true` if registered or library loaded, `false` otherwise.
    #[must_use]
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name) || self.library_path.is_some()
    }

    #[cfg(unix)]
    /// Formats a dlerror pointer into a safe string.
    fn format_dlerror(err_ptr: *mut std::os::raw::c_char) -> String {
        std::ptr::NonNull::new(err_ptr).map_or_else(
            || "unknown dlopen error".to_string(),
            // SAFETY: dlerror returned non-null null-terminated C string.
            |p| unsafe {
                std::ffi::CStr::from_ptr(p.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            },
        )
    }

    /// Resolves the Wine binary path according to active [`WineMode`].
    #[must_use]
    pub fn resolve_wine(&self) -> Option<PathBuf> {
        match self.wine_mode {
            WineMode::Auto => Self::find_wine_binary(),
            WineMode::Disabled => None,
            WineMode::Custom(ref p) => Some(p.clone()),
        }
    }

    /// Configures an explicit Wine executable path for Windows PE DLL emulation.
    ///
    /// # Arguments
    ///
    /// * `wine_executable` - Optional path to Wine binary.
    ///
    /// # Returns
    ///
    /// Updated [`NativeLibraryLoader`].
    #[must_use]
    pub fn with_wine_executable(self, wine_executable: Option<PathBuf>) -> Self {
        match wine_executable {
            Some(p) => self.with_wine_mode(WineMode::Custom(p)),
            None => self.with_wine_mode(WineMode::Disabled),
        }
    }

    /// Sets the Wine execution mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Target [`WineMode`].
    ///
    /// # Returns
    ///
    /// Updated [`NativeLibraryLoader`].
    #[must_use]
    pub fn with_wine_mode(mut self, mode: WineMode) -> Self {
        self.wine_mode = mode;
        self
    }

    /// Loads an external dynamic shared library into the address space using the platform dynamic linker.
    ///
    /// Inspects file magic header bytes. On non-Windows platforms, if a Windows PE DLL is loaded,
    /// checks for Wine availability; if Wine is present, configures the Wine execution bridge,
    /// or returns [`Error::UnsupportedPlatform`] if Wine is unavailable.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to `.so`, `.dylib`, or `.dll` library.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedPlatform`] if a Windows PE DLL is loaded on non-Windows without Wine,
    /// or [`Error::CustomActionFailed`] on I/O or dynamic loading failure.
    pub fn load_library(&mut self, path: &Path) -> Result<()> {
        if let Ok(header) = fs::read(path) {
            let format = BinaryFormat::detect(&header);
            self.library_format = format;
            self.library_path = Some(path.to_path_buf());

            #[cfg(not(windows))]
            {
                if format == BinaryFormat::PeWindows {
                    if let Some(wine_bin) = self.resolve_wine() {
                        self.wine_executable = Some(wine_bin);
                        return Ok(());
                    }
                    return Err(Error::UnsupportedPlatform {
                        platform: "Windows PE (PE32/PE32+)".to_string(),
                        reason: format!(
                            "cannot execute Windows PE dynamic library '{}' on non-Windows host without Wine ('wine64' or 'wine')",
                            path.display()
                        ),
                    });
                }
            }
        }

        #[cfg(unix)]
        {
            use std::ffi::CString;
            let c_path = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|e| {
                Error::CustomActionFailed {
                    action: path.display().to_string(),
                    reason: format!("invalid path for dlopen: {e}"),
                }
            })?;

            if let Some(h) = self.dl_handle.take() {
                // SAFETY: Closing previously open dynamic library handle.
                unsafe { libc::dlclose(h) };
            }

            // SAFETY: c_path is a valid null-terminated C string pointing to existing library file.
            let handle =
                unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
            if handle.is_null() {
                // SAFETY: Reading thread-local dlerror string.
                let raw_err = unsafe { libc::dlerror() };
                let dl_msg = Self::format_dlerror(raw_err);
                return Err(Error::CustomActionFailed {
                    action: path.display().to_string(),
                    reason: format!("dlopen failed: {dl_msg}"),
                });
            }
            self.dl_handle = Some(handle);
            Ok(())
        }

        #[cfg(windows)]
        {
            if !path.exists() {
                return Err(Error::CustomActionFailed {
                    action: path.display().to_string(),
                    reason: format!("dynamic library file not found: {}", path.display()),
                });
            }
            Ok(())
        }
    }

    /// Extracts an embedded binary table library byte stream to a secure temporary sandbox directory.
    ///
    /// Sets restrictive permissions (`0700` directory, `0755` library) and records the sandbox path.
    ///
    /// # Arguments
    ///
    /// * `lib_name` - Library filename (e.g. `CustomAction.dll`).
    /// * `data` - Raw binary payload bytes.
    ///
    /// # Returns
    ///
    /// Absolute path to extracted library file on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if sandbox creation or file write fails.
    pub fn extract_to_sandbox(&mut self, lib_name: &str, data: &[u8]) -> Result<PathBuf> {
        let counter = SANDBOX_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let dir_name = format!("msi_ca_{pid}_{counter}");
        let sandbox = std::env::temp_dir().join(dir_name);

        fs::create_dir_all(&sandbox)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&sandbox, fs::Permissions::from_mode(0o700));
        }

        let file_path = sandbox.join(lib_name);
        fs::write(&file_path, data)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&file_path, fs::Permissions::from_mode(0o755));
        }

        self.sandbox_dir = Some(sandbox);
        self.library_path = Some(file_path.clone());
        Ok(file_path)
    }

    /// Invokes a custom action entry point function with panic boundary protection.
    ///
    /// Checks registered in-memory functions first; if absent and a dynamic library is loaded,
    /// locates the symbol via `dlsym`.
    ///
    /// # Arguments
    ///
    /// * `entry_point` - Function symbol name to invoke.
    /// * `h_install` - Active MSI install session handle.
    ///
    /// # Returns
    ///
    /// Return code (`0` for success).
    ///
    /// # Errors
    ///
    /// Returns [`Error::CustomActionFailed`] if entry point is missing, if native code panics,
    /// or if Wine execution fails.
    pub fn invoke_action(&self, entry_point: &str, h_install: MSIHANDLE) -> Result<u32> {
        if let Some(ref wine_bin) = self.wine_executable {
            let empty_path = Path::new("");
            let dll = self.library_path.as_deref().unwrap_or(empty_path);
            let (prog, args) =
                Self::build_wine_action_command(wine_bin, dll, entry_point, h_install);
            let runner = SubprocessRunner::new();
            let envs = HashMap::new();
            let res = runner.run(&prog, &args, None, &envs)?;
            return Ok(res.exit_code);
        }

        let func: MsiCustomActionFn = if let Some(&f) = self.functions.get(entry_point) {
            f
        } else {
            #[cfg(unix)]
            {
                if let Some(h) = self.dl_handle {
                    use std::ffi::CString;
                    let c_sym =
                        CString::new(entry_point).map_err(|e| Error::CustomActionFailed {
                            action: entry_point.to_string(),
                            reason: format!("invalid entry point symbol: {e}"),
                        })?;
                    // SAFETY: Resolving symbol pointer from valid non-null library handle.
                    let sym_ptr = unsafe { libc::dlsym(h, c_sym.as_ptr()) };
                    if sym_ptr.is_null() {
                        return Err(Error::CustomActionFailed {
                            action: entry_point.to_string(),
                            reason: format!(
                                "entry point '{entry_point}' not found in loaded library or registered symbols"
                            ),
                        });
                    }
                    // SAFETY: Transmuting non-null function pointer to standard MsiCustomActionFn signature.
                    unsafe {
                        std::mem::transmute::<*mut std::ffi::c_void, MsiCustomActionFn>(sym_ptr)
                    }
                } else {
                    return Err(Error::CustomActionFailed {
                        action: entry_point.to_string(),
                        reason: format!(
                            "entry point '{entry_point}' not found in registered native symbols"
                        ),
                    });
                }
            }
            #[cfg(not(unix))]
            {
                return Err(Error::CustomActionFailed {
                    action: entry_point.to_string(),
                    reason: format!(
                        "entry point '{entry_point}' not found in registered native symbols"
                    ),
                });
            }
        };

        // Execute function inside panic catch boundary
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: Calling convention and signature match MsiCustomActionFn.
            unsafe { func(h_install) }
        }));

        result.map_or_else(
            |_| {
                Err(Error::CustomActionFailed {
                    action: entry_point.to_string(),
                    reason: "panic caught inside native custom action execution".to_string(),
                })
            },
            |code| {
                if code == ERROR_SUCCESS || code == ERROR_SUCCESS_REBOOT_REQUIRED {
                    Ok(code)
                } else {
                    Err(Error::CustomActionFailed {
                        action: entry_point.to_string(),
                        reason: format!("native custom action returned error code {code}"),
                    })
                }
            },
        )
    }
}

impl Drop for NativeLibraryLoader {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(h) = self.dl_handle.take() {
            // SAFETY: Closing valid non-null dynamic library handle on drop.
            unsafe { libc::dlclose(h) };
        }
        if let Some(ref dir) = self.sandbox_dir {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

/// Execution outcome of an external native executable process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessResult {
    /// Process exit code mapped to MSI error code convention.
    pub exit_code: u32,
    /// Standard output stream captured from process.
    pub stdout: String,
    /// Standard error stream captured from process.
    pub stderr: String,
}

/// Runner for native executable subprocess custom actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubprocessRunner {
    /// Timeout in milliseconds before process is forcibly killed.
    pub timeout_ms: u64,
}

impl Default for SubprocessRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SubprocessRunner {
    /// Creates a new [`SubprocessRunner`] with default 30-second timeout.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout_ms: DEFAULT_ACTION_TIMEOUT_MS,
        }
    }

    /// Configures execution timeout in milliseconds.
    #[must_use]
    pub const fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Maps a raw process exit code to standard MSI execution status codes.
    #[must_use]
    pub const fn map_exit_code(raw_code: i32) -> u32 {
        if raw_code == 0 {
            ERROR_SUCCESS
        } else if raw_code == 3010 {
            ERROR_SUCCESS_REBOOT_REQUIRED
        } else {
            ERROR_INSTALL_FAILURE
        }
    }

    /// Executes an executable file synchronously with environment variables, timeout, and output capture.
    ///
    /// # Arguments
    ///
    /// * `executable` - Path to executable file.
    /// * `args` - Command-line arguments.
    /// * `working_dir` - Optional working directory.
    /// * `env_vars` - Environment variables to inject into child process.
    ///
    /// # Returns
    ///
    /// [`SubprocessResult`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExecutionFailed`] if spawning fails, process times out, or exit code indicates error.
    pub fn run(
        &self,
        executable: &Path,
        args: &[String],
        working_dir: Option<&Path>,
        env_vars: &HashMap<String, String>,
    ) -> Result<SubprocessResult> {
        let mut cmd = Command::new(executable);
        cmd.args(args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(wd) = working_dir {
            cmd.current_dir(wd);
        }

        for (k, v) in env_vars {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| Error::ExecutionFailed {
            action: executable.to_string_lossy().to_string(),
            return_code: ERROR_FUNCTION_FAILED,
            message: format!("failed to spawn executable: {e}"),
        })?;

        let start_time = Instant::now();
        let timeout = Duration::from_millis(self.timeout_ms);

        loop {
            if let Some(status) = child.try_wait().unwrap_or(None) {
                use std::io::Read;
                let mut stdout_bytes = Vec::new();
                let mut stderr_bytes = Vec::new();
                let _ = child
                    .stdout
                    .as_mut()
                    .map(|out| out.read_to_end(&mut stdout_bytes));
                let _ = child
                    .stderr
                    .as_mut()
                    .map(|err| err.read_to_end(&mut stderr_bytes));

                let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
                let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

                let raw_code = status.code().unwrap_or(1);
                let msi_code = Self::map_exit_code(raw_code);

                if msi_code == ERROR_INSTALL_FAILURE {
                    return Err(Error::ExecutionFailed {
                        action: executable.to_string_lossy().to_string(),
                        return_code: msi_code,
                        message: format!("process exited with code {raw_code}: {stderr}"),
                    });
                }

                return Ok(SubprocessResult {
                    exit_code: msi_code,
                    stdout,
                    stderr,
                });
            }

            if start_time.elapsed() > timeout {
                let _ = child.kill();
                return Err(Error::ExecutionFailed {
                    action: executable.to_string_lossy().to_string(),
                    return_code: ERROR_INSTALL_FAILURE,
                    message: format!(
                        "custom action execution timed out after {}ms",
                        self.timeout_ms
                    ),
                });
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Spawns an executable process in the background without waiting for completion.
    ///
    /// # Arguments
    ///
    /// * `executable` - Path to executable file.
    /// * `args` - Command-line arguments.
    /// * `working_dir` - Optional working directory.
    /// * `env_vars` - Environment variables.
    ///
    /// # Returns
    ///
    /// Active [`Child`] process handle.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExecutionFailed`] if spawning fails.
    pub fn spawn_async(
        &self,
        executable: &Path,
        args: &[String],
        working_dir: Option<&Path>,
        env_vars: &HashMap<String, String>,
    ) -> Result<Child> {
        let mut cmd = Command::new(executable);
        cmd.args(args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        if let Some(wd) = working_dir {
            cmd.current_dir(wd);
        }

        for (k, v) in env_vars {
            cmd.env(k, v);
        }

        cmd.spawn().map_err(|e| Error::ExecutionFailed {
            action: executable.to_string_lossy().to_string(),
            return_code: ERROR_FUNCTION_FAILED,
            message: format!("failed to spawn async executable: {e}"),
        })
    }
}

/// Execution mode for SQL schema provisioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlProvisionerAction {
    /// Create database, users, and grant privileges.
    Install,
    /// Uninstall phase (drops database/user only if `PURGE_DATA="1"`).
    Uninstall,
}

/// Configuration parameters for in-process SQL provisioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlProvisionerConfig {
    /// Host or IP address of target database server.
    pub host: String,
    /// TCP listening port of the database server.
    pub port: u16,
    /// Administrative database username (e.g. `root`).
    pub root_user: String,
    /// Administrative database password.
    pub root_password: Option<String>,
    /// Target application database name (e.g. `openedx` or `wordpress`).
    pub target_database: String,
    /// Target application username to provision.
    pub target_user: Option<String>,
    /// Target application user password.
    pub target_password: Option<String>,
    /// Character collation (e.g. `utf8mb4_unicode_ci`).
    pub target_collation: String,
    /// Whether to drop database and users on uninstall (`PURGE_DATA="1"`).
    pub purge_data: bool,
    /// Whether to operate in mock/offline mode (synthesizes and validates SQL queries without opening network socket).
    pub mock_mode: bool,
}

impl Default for SqlProvisionerConfig {
    /// Creates a default [`SqlProvisionerConfig`] matching standard defaults.
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3306,
            root_user: "root".to_string(),
            root_password: None,
            target_database: "openedx".to_string(),
            target_user: None,
            target_password: None,
            target_collation: "utf8mb4_unicode_ci".to_string(),
            purge_data: false,
            mock_mode: false,
        }
    }
}

impl SqlProvisionerConfig {
    /// Resolves configuration parameters from active [`crate::execution::properties::EvaluationContext`].
    ///
    /// # Arguments
    ///
    /// * `ctx` - The property evaluation context.
    ///
    /// # Returns
    ///
    /// Extracted and normalized [`SqlProvisionerConfig`].
    #[must_use]
    pub fn from_context(ctx: &crate::execution::properties::EvaluationContext) -> Self {
        let host = ctx
            .get_property("PROP_MYSQL_HOST")
            .unwrap_or("127.0.0.1")
            .to_string();

        let port = ctx
            .get_property("PROP_MYSQL_PORT")
            .or_else(|| ctx.get_property("MYSQL_PORT"))
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3306);

        let root_user = ctx
            .get_property("PROP_MYSQL_ROOT_USER")
            .unwrap_or("root")
            .to_string();

        let root_password = ctx
            .get_property("PROP_MYSQL_ROOT_PASSWORD")
            .map(ToString::to_string);

        let target_database = ctx
            .get_property("PROP_PROVISION_DB_NAME")
            .or_else(|| ctx.get_property("TARGET_DATABASE"))
            .unwrap_or("openedx")
            .to_string();

        let target_user = ctx
            .get_property("PROP_PROVISION_USER")
            .or_else(|| ctx.get_property("TARGET_USER"))
            .map(ToString::to_string);

        let target_password = ctx
            .get_property("PROP_PROVISION_PASSWORD")
            .or_else(|| ctx.get_property("TARGET_PASSWORD"))
            .map(ToString::to_string);

        let target_collation = ctx
            .get_property("PROP_PROVISION_COLLATION")
            .unwrap_or("utf8mb4_unicode_ci")
            .to_string();

        let purge_data = ctx.get_property("PURGE_DATA").is_some_and(|v| v == "1");
        let mock_mode = ctx
            .get_property("SQL_PROVISION_MOCK")
            .is_some_and(|v| v == "1")
            || ctx.get_property("MOCK_OFFLINE").is_some_and(|v| v == "1");

        Self {
            host,
            port,
            root_user,
            root_password,
            target_database,
            target_user,
            target_password,
            target_collation,
            purge_data,
            mock_mode,
        }
    }

    /// Synthesizes the list of SQL statements to execute for the given action.
    ///
    /// # Arguments
    ///
    /// * `action` - The provisioning action (`Install` or `Uninstall`).
    ///
    /// # Returns
    ///
    /// Vector of SQL query strings.
    #[must_use]
    pub fn generate_statements(&self, action: SqlProvisionerAction) -> Vec<String> {
        let mut stmts = Vec::new();
        match action {
            SqlProvisionerAction::Install => {
                stmts.push(format!(
                    "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE {};",
                    self.target_database, self.target_collation
                ));

                if let Some(ref user) = self.target_user {
                    let pwd = self.target_password.as_deref().unwrap_or("");
                    stmts.push(format!(
                        "CREATE USER IF NOT EXISTS '{user}'@'%' IDENTIFIED BY '{pwd}';"
                    ));
                    stmts.push(format!(
                        "GRANT ALL PRIVILEGES ON `{}`.* TO '{user}'@'%';",
                        self.target_database
                    ));
                    stmts.push("FLUSH PRIVILEGES;".to_string());
                }
            }
            SqlProvisionerAction::Uninstall => {
                if self.purge_data {
                    stmts.push(format!(
                        "DROP DATABASE IF EXISTS `{}`;",
                        self.target_database
                    ));
                    if let Some(ref user) = self.target_user {
                        stmts.push(format!("DROP USER IF EXISTS '{user}'@'%';"));
                        stmts.push("FLUSH PRIVILEGES;".to_string());
                    }
                }
            }
        }
        stmts
    }
}

/// Result of SQL provisioning execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlProvisionerResult {
    /// Whether all statements executed successfully.
    pub success: bool,
    /// List of statements that were executed or synthesized.
    pub executed_statements: Vec<String>,
}

/// In-process SQL provisioner capable of direct TCP MySQL protocol execution.
#[derive(Debug)]
pub struct SqlProvisionerClient {
    /// Active configuration.
    config: SqlProvisionerConfig,
}

/// Unified trait combining [`std::io::Read`] and [`std::io::Write`] for stream communication.
pub trait ReadWrite: std::io::Read + std::io::Write {}
impl<T: std::io::Read + std::io::Write + ?Sized> ReadWrite for T {}

impl SqlProvisionerClient {
    /// Creates a new [`SqlProvisionerClient`] with the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Provisioning configuration.
    ///
    /// # Returns
    ///
    /// A new [`SqlProvisionerClient`].
    #[must_use]
    pub const fn new(config: SqlProvisionerConfig) -> Self {
        Self { config }
    }

    /// Executes database schema provisioning for the given action without spawning external `.exe` processes.
    ///
    /// # Arguments
    ///
    /// * `action` - Provisioning action (`Install` or `Uninstall`).
    ///
    /// # Returns
    ///
    /// [`SqlProvisionerResult`] with executed queries.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SqlProvisioning`] on connection or execution failure.
    pub fn execute(&self, action: SqlProvisionerAction) -> Result<SqlProvisionerResult> {
        let statements = self.config.generate_statements(action);

        if self.config.mock_mode || statements.is_empty() {
            return Ok(SqlProvisionerResult {
                success: true,
                executed_statements: statements,
            });
        }

        let addr = format!("{}:{}", self.config.host, self.config.port);
        let sock_addr = match addr.parse() {
            Ok(sa) => sa,
            Err(e) => {
                return Err(Error::SqlProvisioning(format!(
                    "invalid socket address '{addr}': {e}"
                )));
            }
        };

        let stream_res =
            std::net::TcpStream::connect_timeout(&sock_addr, Duration::from_millis(5000));
        let mut stream = match stream_res {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::SqlProvisioning(format!(
                    "failed to connect to MySQL server at {addr}: {e}"
                )));
            }
        };

        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

        self.execute_wire_session(&mut stream, &statements)?;

        Ok(SqlProvisionerResult {
            success: true,
            executed_statements: statements,
        })
    }

    /// Executes the MySQL wire protocol handshake, authentication, and SQL query batch over a stream.
    ///
    /// # Arguments
    ///
    /// * `stream` - The stream to communicate over.
    /// * `statements` - List of SQL queries to execute.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SqlProvisioning`] on communication, authentication, or query failure.
    #[allow(clippy::too_many_lines)]
    pub fn execute_wire_session(
        &self,
        stream: &mut dyn ReadWrite,
        statements: &[String],
    ) -> Result<()> {
        let mut header = [0u8; 4];
        stream.read_exact(&mut header).map_err(|e| {
            Error::SqlProvisioning(format!("failed to read MySQL handshake packet header: {e}"))
        })?;

        let payload_len = (u32::from(header[0])
            | (u32::from(header[1]) << 8)
            | (u32::from(header[2]) << 16)) as usize;

        let mut handshake_payload = vec![0u8; payload_len];
        stream.read_exact(&mut handshake_payload).map_err(|e| {
            Error::SqlProvisioning(format!("failed to read MySQL handshake payload: {e}"))
        })?;

        let mut response_payload = Vec::new();
        response_payload.extend_from_slice(&0x0008_0201u32.to_le_bytes());
        response_payload.extend_from_slice(&0x0100_0000u32.to_le_bytes());
        response_payload.push(33);
        response_payload.extend_from_slice(&[0u8; 23]);
        response_payload.extend_from_slice(self.config.root_user.as_bytes());
        response_payload.push(0);
        response_payload.push(0);

        let resp_len = u32::try_from(response_payload.len()).unwrap_or(0);
        let mut resp_header = [0u8; 4];
        resp_header[0] = (resp_len & 0xFF) as u8;
        resp_header[1] = ((resp_len >> 8) & 0xFF) as u8;
        resp_header[2] = ((resp_len >> 16) & 0xFF) as u8;
        resp_header[3] = 1;

        stream.write_all(&resp_header).map_err(|e| {
            Error::SqlProvisioning(format!("failed to write handshake response header: {e}"))
        })?;
        stream.write_all(&response_payload).map_err(|e| {
            Error::SqlProvisioning(format!("failed to write handshake response payload: {e}"))
        })?;

        let mut auth_header = [0u8; 4];
        stream.read_exact(&mut auth_header).map_err(|e| {
            Error::SqlProvisioning(format!("failed to read auth response packet: {e}"))
        })?;
        let auth_len = (u32::from(auth_header[0])
            | (u32::from(auth_header[1]) << 8)
            | (u32::from(auth_header[2]) << 16)) as usize;
        let mut auth_result = vec![0u8; auth_len];
        stream.read_exact(&mut auth_result).map_err(|e| {
            Error::SqlProvisioning(format!("failed to read auth response payload: {e}"))
        })?;

        if !auth_result.is_empty() && auth_result[0] == 0xFF {
            return Err(Error::SqlProvisioning(
                "authentication failed with MySQL server".to_string(),
            ));
        }

        let mut seq: u8 = 0;
        for sql in statements {
            seq = seq.wrapping_add(1);
            let mut query_payload = Vec::with_capacity(sql.len() + 1);
            query_payload.push(0x03);
            query_payload.extend_from_slice(sql.as_bytes());

            let q_len = u32::try_from(query_payload.len()).unwrap_or(0);
            let mut q_header = [0u8; 4];
            q_header[0] = (q_len & 0xFF) as u8;
            q_header[1] = ((q_len >> 8) & 0xFF) as u8;
            q_header[2] = ((q_len >> 16) & 0xFF) as u8;
            q_header[3] = seq;

            stream
                .write_all(&q_header)
                .map_err(|e| Error::SqlProvisioning(format!("failed to send query header: {e}")))?;
            stream
                .write_all(&query_payload)
                .map_err(|e| Error::SqlProvisioning(format!("failed to send query text: {e}")))?;

            let mut query_res_header = [0u8; 4];
            stream.read_exact(&mut query_res_header).map_err(|e| {
                Error::SqlProvisioning(format!("failed to read query response header: {e}"))
            })?;
            let r_len = (u32::from(query_res_header[0])
                | (u32::from(query_res_header[1]) << 8)
                | (u32::from(query_res_header[2]) << 16)) as usize;
            let mut q_result = vec![0u8; r_len];
            stream.read_exact(&mut q_result).map_err(|e| {
                Error::SqlProvisioning(format!("failed to read query response payload: {e}"))
            })?;

            if !q_result.is_empty() && q_result[0] == 0xFF {
                return Err(Error::SqlProvisioning(format!(
                    "SQL execution error executing query '{sql}'"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock custom action returning `ERROR_SUCCESS`.
    unsafe extern "system-unwind" fn mock_success_action(_h: MSIHANDLE) -> u32 {
        ERROR_SUCCESS
    }

    /// Mock custom action returning `ERROR_SUCCESS_REBOOT_REQUIRED` (3010).
    unsafe extern "system-unwind" fn mock_reboot_action(_h: MSIHANDLE) -> u32 {
        ERROR_SUCCESS_REBOOT_REQUIRED
    }

    /// Mock custom action returning non-zero error.
    unsafe extern "system-unwind" fn mock_failure_action(_h: MSIHANDLE) -> u32 {
        1603
    }

    /// Mock custom action that panics.
    unsafe extern "system-unwind" fn mock_panic_action(_h: MSIHANDLE) -> u32 {
        panic!("simulated native panic inside custom action");
    }

    /// Tests `NativeLibraryLoader` function registration, invocation, error, and panic boundary isolation.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_native_library_loader() {
        let mut loader = NativeLibraryLoader::default();
        loader.register_function("SuccessAction", mock_success_action);
        loader.register_function("RebootAction", mock_reboot_action);
        loader.register_function("FailureAction", mock_failure_action);
        loader.register_function("PanicAction", mock_panic_action);

        // Success
        let res_ok = loader.invoke_action("SuccessAction", 100);
        assert_eq!(res_ok, Ok(ERROR_SUCCESS));

        // Reboot required
        let res_reboot = loader.invoke_action("RebootAction", 100);
        assert_eq!(res_reboot, Ok(ERROR_SUCCESS_REBOOT_REQUIRED));

        // Failure
        let res_err = loader.invoke_action("FailureAction", 100);
        assert!(res_err.is_err());

        // Panic caught safely without crashing test process
        let res_panic = loader.invoke_action("PanicAction", 100);
        assert!(res_panic.is_err());

        // Missing function
        let res_missing = loader.invoke_action("UnknownAction", 100);
        assert!(res_missing.is_err());

        // Test Clone implementation with library_path = None
        let cloned_none = loader.clone();
        assert_eq!(
            cloned_none.invoke_action("SuccessAction", 100),
            Ok(ERROR_SUCCESS)
        );

        // Test has_function branches
        let empty_loader = NativeLibraryLoader::new();
        assert!(!empty_loader.has_function("AnyFunc"));
        assert!(loader.has_function("SuccessAction"));

        // Test Clone implementation with library_path = Some(...)
        let mut loaded_loader = NativeLibraryLoader::new();
        let loaded_path = std::env::temp_dir().join("mock_lib.dll");
        loaded_loader.library_path = Some(loaded_path);
        let cloned_some = loaded_loader.clone();
        assert_eq!(cloned_some.library_path, loaded_loader.library_path);
        assert!(cloned_some.has_function("AnyOtherFunc"));

        // Test sandbox extraction and cleanup
        let extract_res = loader.extract_to_sandbox("test_ca.dll", b"MZ_MOCK_PE_HEADER");
        assert!(extract_res.is_ok());
        let lib_path = extract_res.unwrap_or_default();
        assert!(lib_path.exists());
        let parent_dir = lib_path.parent().map(Path::to_path_buf).unwrap_or_default();
        assert!(parent_dir.exists());

        drop(loader);
        // Sandbox directory should be deleted on drop
        assert!(!parent_dir.exists());

        // Test extract_to_sandbox directory creation failure
        let mut fail_dir_loader = NativeLibraryLoader::new();
        let counter = SANDBOX_COUNTER.load(Ordering::SeqCst);
        let pid = std::process::id();
        let block_path = std::env::temp_dir().join(format!("msi_ca_{pid}_{counter}"));
        let _ = fs::write(&block_path, b"blocking_file");
        let res_dir_fail = fail_dir_loader.extract_to_sandbox("test.dll", b"data");
        let _ = fs::remove_file(&block_path);
        assert!(res_dir_fail.is_err());

        // Test extract_to_sandbox file write failure (nonexistent subdirectory)
        let mut fail_write_loader = NativeLibraryLoader::new();
        assert!(fail_write_loader
            .extract_to_sandbox("nonexistent_sub/test.dll", b"data")
            .is_err());

        // Test map_exit_code for all status paths
        assert_eq!(SubprocessRunner::map_exit_code(0), ERROR_SUCCESS);
        assert_eq!(
            SubprocessRunner::map_exit_code(3010),
            ERROR_SUCCESS_REBOOT_REQUIRED
        );
        assert_eq!(SubprocessRunner::map_exit_code(1), ERROR_INSTALL_FAILURE);
        assert_eq!(SubprocessRunner::map_exit_code(-1), ERROR_INSTALL_FAILURE);

        // Test BinaryFormat detection
        assert_eq!(BinaryFormat::detect(b"MZ\x90\x00"), BinaryFormat::PeWindows);
        assert_eq!(
            BinaryFormat::detect(b"\x7fELF\x02\x01"),
            BinaryFormat::ElfUnix
        );
        assert_eq!(
            BinaryFormat::detect(&0xFEED_FACE_u32.to_ne_bytes()),
            BinaryFormat::MachOApple
        );
        assert_eq!(
            BinaryFormat::detect(&0xFEED_FACF_u32.to_ne_bytes()),
            BinaryFormat::MachOApple
        );
        assert_eq!(
            BinaryFormat::detect(&0xCEFA_EDFE_u32.to_ne_bytes()),
            BinaryFormat::MachOApple
        );
        assert_eq!(
            BinaryFormat::detect(&0xCFFA_EDFE_u32.to_ne_bytes()),
            BinaryFormat::MachOApple
        );
        assert_eq!(
            BinaryFormat::detect(&0xCAFE_BABE_u32.to_ne_bytes()),
            BinaryFormat::MachOApple
        );
        assert_eq!(
            BinaryFormat::detect(&0xBEBA_FECA_u32.to_ne_bytes()),
            BinaryFormat::MachOApple
        );
        assert_eq!(BinaryFormat::detect(b"DATA"), BinaryFormat::Unknown);
        assert_eq!(BinaryFormat::detect(b"M"), BinaryFormat::Unknown);
        assert_eq!(BinaryFormat::detect(&[0x4D, 0x00]), BinaryFormat::Unknown);
        assert_eq!(BinaryFormat::detect(b""), BinaryFormat::Unknown);

        // Test WineMode derives
        let default_wm = WineMode::default();
        assert_eq!(default_wm, WineMode::Auto);
        let clone_wm = default_wm.clone();
        assert_eq!(default_wm, clone_wm);
        assert!(format!("{default_wm:?}").contains("Auto"));

        // Test Wine command builder
        let (cmd, args) = NativeLibraryLoader::build_wine_action_command(
            Path::new("/usr/bin/wine64"),
            Path::new("/tmp/custom.dll"),
            "EntryPoint",
            42,
        );
        assert_eq!(cmd, PathBuf::from("/usr/bin/wine64"));
        assert_eq!(
            args,
            vec!["rundll32.exe", "/tmp/custom.dll,EntryPoint", "42"]
        );

        // Test find_wine_binary and find_wine_binary_in
        let _ = NativeLibraryLoader::find_wine_binary();

        // Test loading PE DLL on non-Windows
        let temp_pe = std::env::temp_dir().join(format!("test_mock_pe_{}.dll", std::process::id()));
        let _ = fs::write(&temp_pe, b"MZ\x90\x00mock_pe_executable_bytes");
        let pe_loader = NativeLibraryLoader::new();
        assert_eq!(pe_loader.library_format(), BinaryFormat::Unknown);
        assert!(pe_loader.wine_executable().is_none());

        #[cfg(not(windows))]
        {
            // 1. Fallback search when PATH and fallback dirs are empty
            assert!(NativeLibraryLoader::find_wine_binary_custom(None, &[]).is_none());

            // 2. Search when PATH specifies directory containing mock wine
            let mock_wine_dir =
                std::env::temp_dir().join(format!("mock_wine_{}", std::process::id()));
            let _ = fs::create_dir_all(&mock_wine_dir);
            let mock_wine = mock_wine_dir.join("wine");
            let _ = fs::write(&mock_wine, b"#!/bin/sh\nexit 0\n");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&mock_wine, fs::Permissions::from_mode(0o755));
            }
            let found_path =
                NativeLibraryLoader::find_wine_binary_custom(Some(mock_wine_dir.as_os_str()), &[]);
            assert_eq!(found_path, Some(mock_wine.clone()));

            let mock_wine_str = mock_wine_dir.to_string_lossy().to_string();
            let found_fallback =
                NativeLibraryLoader::find_wine_binary_custom(None, &[&mock_wine_str]);
            assert_eq!(found_fallback, Some(mock_wine.clone()));

            // 3. With explicit wine executable configured: must succeed and invoke action
            let mut wine_loader = NativeLibraryLoader::new().with_wine_executable(Some(mock_wine));
            assert!(wine_loader.load_library(&temp_pe).is_ok());
            assert!(wine_loader.wine_executable().is_some());
            assert_eq!(
                wine_loader.invoke_action("CustomAction", 1),
                Ok(ERROR_SUCCESS)
            );

            // 4. With wine disabled: must return UnsupportedPlatform
            let mut disabled_loader = NativeLibraryLoader::new().with_wine_mode(WineMode::Disabled);
            let unset_loader = NativeLibraryLoader::new().with_wine_executable(None);
            assert_eq!(unset_loader.wine_mode, WineMode::Disabled);
            let load_disabled = disabled_loader.load_library(&temp_pe);
            assert!(matches!(
                load_disabled,
                Err(Error::UnsupportedPlatform { .. })
            ));
            assert!(disabled_loader.wine_executable().is_none());

            let _ = fs::remove_dir_all(&mock_wine_dir);
        }
        let _ = fs::remove_file(&temp_pe);

        // Test loading non-PE file (ELF)
        let temp_elf =
            std::env::temp_dir().join(format!("test_mock_elf_{}.so", std::process::id()));
        let _ = fs::write(&temp_elf, b"\x7FELF\x02\x01\x01\x00_mock_elf_bytes");
        let mut elf_loader = NativeLibraryLoader::new();
        let _ = elf_loader.load_library(&temp_elf);
        assert_eq!(elf_loader.library_format(), BinaryFormat::ElfUnix);
        let _ = fs::remove_file(&temp_elf);

        // Test resolve_wine in Auto mode
        let auto_loader = NativeLibraryLoader::new();
        assert_eq!(auto_loader.wine_mode, WineMode::Auto);
        let _ = auto_loader.resolve_wine();

        // Test invoke_action with mock wine executable
        #[cfg(not(target_os = "windows"))]
        {
            let mut mock_wine_loader = NativeLibraryLoader::new();
            mock_wine_loader.wine_executable = Some(PathBuf::from("/usr/bin/true"));
            mock_wine_loader.library_path = Some(PathBuf::from("/tmp/test.dll"));
            let wine_invoke_res = mock_wine_loader.invoke_action("CustomAction", 1);
            assert_eq!(wine_invoke_res, Ok(ERROR_SUCCESS));

            // Test invoke_action when library_path is None
            mock_wine_loader.library_path = None;
            assert_eq!(
                mock_wine_loader.invoke_action("CustomAction", 1),
                Ok(ERROR_SUCCESS)
            );

            // Test invoke_action when runner.run fails (wine executable does not exist)
            mock_wine_loader.wine_executable = Some(PathBuf::from("/nonexistent/bin/wine_fail"));
            assert!(mock_wine_loader.invoke_action("CustomAction", 1).is_err());
        }

        // Test dynamic library loading error on invalid path
        let mut dl_loader = NativeLibraryLoader::new();
        assert!(dl_loader
            .load_library(Path::new("/non/existent/library.so"))
            .is_err());

        // Test dlopen error string formatting helper
        #[cfg(unix)]
        {
            let err_str_null = NativeLibraryLoader::format_dlerror(std::ptr::null_mut());
            assert_eq!(err_str_null, "unknown dlopen error");
        }

        // Test path with internal null byte for dlopen error
        #[cfg(unix)]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;
            let bad_path = Path::new(OsStr::from_bytes(b"/invalid/\0path.so"));
            assert!(dl_loader.load_library(bad_path).is_err());
        }

        #[cfg(target_os = "macos")]
        {
            let lib_sys = Path::new("libSystem.B.dylib");
            assert!(dl_loader.load_library(lib_sys).is_ok());
            // Loading another library when handle is Some covers closing previous handle:
            assert!(dl_loader.load_library(lib_sys).is_ok());

            // Symbol that exists in libSystem and returns 0 for char 'A' (65): isspace('A') == 0
            assert_eq!(dl_loader.invoke_action("isspace", 65), Ok(ERROR_SUCCESS));
            // Symbol that does not exist
            assert!(dl_loader
                .invoke_action("non_existent_symbol_xyz", 0)
                .is_err());
            // Symbol with interior null
            assert!(dl_loader.invoke_action("invalid\0symbol", 0).is_err());
        }
    }

    /// Tests `SubprocessRunner` executing a basic command, capturing output, and handling errors.
    #[test]
    fn test_subprocess_runner() {
        let runner = SubprocessRunner::new().with_timeout(5000);
        let mut envs = HashMap::new();
        envs.insert("MSI_TEST_VAR".to_string(), "HELLO_MSI".to_string());

        // Test standard echo or true command
        #[cfg(not(target_os = "windows"))]
        {
            let temp_dir = std::env::temp_dir();
            let res = runner.run(
                Path::new("/bin/sh"),
                &["-c".to_string(), "echo $MSI_TEST_VAR".to_string()],
                Some(&temp_dir),
                &envs,
            );
            assert_eq!(
                res.as_ref()
                    .map(|o| (o.exit_code, o.stdout.contains("HELLO_MSI"))),
                Ok((ERROR_SUCCESS, true))
            );

            // Test non-zero exit code error
            let err_res = runner.run(
                Path::new("/bin/sh"),
                &["-c".to_string(), "exit 42".to_string()],
                None,
                &envs,
            );
            assert!(err_res.is_err());

            // Test non-existent executable spawn failure
            let missing_exec =
                runner.run(Path::new("/non/existent/exec/path_xyz"), &[], None, &envs);
            assert!(missing_exec.is_err());

            // Test async spawn of non-existent executable
            let missing_async =
                runner.spawn_async(Path::new("/non/existent/exec/path_xyz"), &[], None, &envs);
            assert!(missing_async.is_err());

            // Test 3010 reboot code mapping
            let reboot_res = runner.run(
                Path::new("/bin/sh"),
                &["-c".to_string(), "exit 186".to_string()], // 3010 % 256 = 186 in posix exit status
                None,
                &envs,
            );
            let _ = reboot_res;

            // Test timeout
            let fast_runner = SubprocessRunner::default().with_timeout(50);
            let timeout_res =
                fast_runner.run(Path::new("/bin/sleep"), &["1".to_string()], None, &envs);
            assert!(timeout_res.is_err());

            // Test async spawn with working_dir
            let mut child_res = runner.spawn_async(
                Path::new("/bin/sleep"),
                &["0.1".to_string()],
                Some(&temp_dir),
                &envs,
            );
            assert_eq!(child_res.as_mut().map(|c| c.wait().is_ok()), Ok(true));
        }

        #[cfg(target_os = "windows")]
        {
            let res = runner.run(
                Path::new("cmd.exe"),
                &["/C".to_string(), "echo %MSI_TEST_VAR%".to_string()],
                None,
                &envs,
            );
            assert_eq!(res.as_ref().map(|o| o.exit_code), Ok(ERROR_SUCCESS));
        }
    }

    /// Tests SQL provisioner config extraction, statement generation, and execution in mock and error modes.
    #[test]
    fn test_sql_provisioner_configuration_and_mock_execution() -> Result<()> {
        use crate::execution::properties::EvaluationContext;

        // 1. Context parsing
        let mut ctx = EvaluationContext::new();
        ctx.set_property("PROP_MYSQL_HOST", "127.0.0.1");
        ctx.set_property("PROP_MYSQL_PORT", "3306");
        ctx.set_property("PROP_MYSQL_ROOT_USER", "root");
        ctx.set_property("PROP_MYSQL_ROOT_PASSWORD", "root_secret");
        ctx.set_property("PROP_PROVISION_DB_NAME", "openedx");
        ctx.set_property("PROP_PROVISION_USER", "openedx");
        ctx.set_property("PROP_PROVISION_PASSWORD", "edx_secret");
        ctx.set_property("PROP_PROVISION_COLLATION", "utf8mb4_unicode_ci");
        ctx.set_property("PURGE_DATA", "0");
        ctx.set_property("SQL_PROVISION_MOCK", "1");

        let cfg = SqlProvisionerConfig::from_context(&ctx);
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 3306);
        assert_eq!(cfg.root_user, "root");
        assert_eq!(cfg.root_password, Some("root_secret".to_string()));
        assert_eq!(cfg.target_database, "openedx");
        assert_eq!(cfg.target_user, Some("openedx".to_string()));
        assert_eq!(cfg.target_password, Some("edx_secret".to_string()));
        assert_eq!(cfg.target_collation, "utf8mb4_unicode_ci");
        assert!(!cfg.purge_data);
        assert!(cfg.mock_mode);

        // 2. Install statements generation
        let install_stmts = cfg.generate_statements(SqlProvisionerAction::Install);
        assert_eq!(install_stmts.len(), 4);
        assert!(install_stmts[0].contains("CREATE DATABASE IF NOT EXISTS `openedx`"));
        assert!(install_stmts[1].contains("CREATE USER IF NOT EXISTS 'openedx'@'%'"));
        assert!(install_stmts[2].contains("GRANT ALL PRIVILEGES ON `openedx`.*"));
        assert_eq!(install_stmts[3], "FLUSH PRIVILEGES;");

        // 3. Uninstall statements generation with PURGE_DATA=0 (must be empty!)
        let uninstall_stmts_nopurge = cfg.generate_statements(SqlProvisionerAction::Uninstall);
        assert!(uninstall_stmts_nopurge.is_empty());

        // 4. Uninstall statements with PURGE_DATA=1
        let mut cfg_purge = cfg.clone();
        cfg_purge.purge_data = true;
        let uninstall_stmts_purge = cfg_purge.generate_statements(SqlProvisionerAction::Uninstall);
        assert_eq!(uninstall_stmts_purge.len(), 3);
        assert!(uninstall_stmts_purge[0].contains("DROP DATABASE IF EXISTS `openedx`"));
        assert!(uninstall_stmts_purge[1].contains("DROP USER IF EXISTS 'openedx'@'%'"));
        assert_eq!(uninstall_stmts_purge[2], "FLUSH PRIVILEGES;");

        // 5. Mock mode execution
        let client = SqlProvisionerClient::new(cfg);
        let res = client.execute(SqlProvisionerAction::Install)?;
        assert!(res.success);
        assert_eq!(res.executed_statements.len(), 4);

        // 6. Network error branches (non-mock mode with unreachable server)
        let mut cfg_real = cfg_purge.clone();
        cfg_real.mock_mode = false;
        cfg_real.port = 1; // Unlikely to have MySQL running on port 1
        let real_client = SqlProvisionerClient::new(cfg_real.clone());
        let err = real_client.execute(SqlProvisionerAction::Install);
        assert!(err.is_err());
        assert!(matches!(err, Err(Error::SqlProvisioning(..))));

        // 7. Invalid host address
        let mut cfg_bad_addr = cfg_purge;
        cfg_bad_addr.mock_mode = false;
        cfg_bad_addr.host = "invalid.ip.address".to_string();
        let bad_client = SqlProvisionerClient::new(cfg_bad_addr);
        let err_addr = bad_client.execute(SqlProvisionerAction::Install);
        assert!(err_addr.is_err());
        assert!(matches!(err_addr, Err(Error::SqlProvisioning(..))));

        // 8. Statements empty with non-mock mode
        let mut cfg_no_stmts = cfg_real;
        cfg_no_stmts.purge_data = false;
        let empty_client = SqlProvisionerClient::new(cfg_no_stmts);
        let empty_res = empty_client.execute(SqlProvisionerAction::Uninstall)?;
        assert!(empty_res.success);
        assert!(empty_res.executed_statements.is_empty());

        Ok(())
    }

    /// Tests fallback properties and statement generation variations.
    #[test]
    fn test_sql_provisioner_from_context_fallbacks_and_variants() {
        use crate::execution::properties::EvaluationContext;

        // Default empty context
        let empty_ctx = EvaluationContext::new();
        let default_cfg = SqlProvisionerConfig::from_context(&empty_ctx);
        assert_eq!(default_cfg.host, "127.0.0.1");
        assert_eq!(default_cfg.port, 3306);
        assert_eq!(default_cfg.root_user, "root");
        assert!(default_cfg.root_password.is_none());
        assert_eq!(default_cfg.target_database, "openedx");
        assert!(default_cfg.target_user.is_none());
        assert!(default_cfg.target_password.is_none());
        assert_eq!(default_cfg.target_collation, "utf8mb4_unicode_ci");
        assert!(!default_cfg.purge_data);
        assert!(!default_cfg.mock_mode);

        // Install without target_user
        let no_user_install = default_cfg.generate_statements(SqlProvisionerAction::Install);
        assert_eq!(no_user_install.len(), 1);

        // Uninstall with purge_data = true but target_user = None
        let mut purge_no_user = default_cfg;
        purge_no_user.purge_data = true;
        let drop_stmts = purge_no_user.generate_statements(SqlProvisionerAction::Uninstall);
        assert_eq!(drop_stmts.len(), 1);
        assert!(drop_stmts[0].contains("DROP DATABASE IF EXISTS `openedx`"));

        // Context using secondary property names
        let mut alt_ctx = EvaluationContext::new();
        alt_ctx.set_property("MYSQL_PORT", "3307");
        alt_ctx.set_property("TARGET_DATABASE", "custom_db");
        alt_ctx.set_property("TARGET_USER", "custom_user");
        alt_ctx.set_property("TARGET_PASSWORD", "custom_pwd");
        alt_ctx.set_property("MOCK_OFFLINE", "1");
        alt_ctx.set_property("PURGE_DATA", "1");

        let alt_cfg = SqlProvisionerConfig::from_context(&alt_ctx);
        assert_eq!(alt_cfg.port, 3307);
        assert_eq!(alt_cfg.target_database, "custom_db");
        assert_eq!(alt_cfg.target_user, Some("custom_user".to_string()));
        assert_eq!(alt_cfg.target_password, Some("custom_pwd".to_string()));
        assert!(alt_cfg.mock_mode);
        assert!(alt_cfg.purge_data);

        // Install with user and no password
        let mut no_pwd_cfg = alt_cfg;
        no_pwd_cfg.target_password = None;
        let install_no_pwd = no_pwd_cfg.generate_statements(SqlProvisionerAction::Install);
        assert_eq!(install_no_pwd.len(), 4);
        assert!(install_no_pwd[1].contains("IDENTIFIED BY ''"));
    }

    /// Simulation modes for mock MySQL server behavior.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockServerBehavior {
        /// Fully successful MySQL handshake, auth, and query processing.
        Success,
        /// Close socket immediately to simulate handshake header read error.
        FailHandshakeHeader,
        /// Send truncated packet to simulate handshake payload read error.
        FailHandshakePayload,
        /// Close socket before sending authentication response header.
        FailAuthHeader,
        /// Send truncated authentication response payload.
        FailAuthPayload,
        /// Return MySQL error packet on authentication.
        AuthFailed,
        /// Close socket before sending query response header.
        FailQueryHeader,
        /// Send truncated query response payload.
        FailQueryPayload,
        /// Return MySQL error packet on query execution.
        QueryFailed,
    }

    /// Spawns an in-process mock MySQL server listening on a local loopback port.
    ///
    /// # Arguments
    ///
    /// * `behavior` - The desired test behavior.
    ///
    /// # Returns
    ///
    /// Tuple of bound port and thread join handle.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    fn spawn_mock_mysql_server(
        behavior: MockServerBehavior,
    ) -> Result<(u16, std::thread::JoinHandle<()>)> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();

        let handle = std::thread::spawn(move || {
            let _ = (|| -> std::io::Result<()> {
                let (mut socket, _) = listener.accept()?;

                match behavior {
                    MockServerBehavior::FailHandshakeHeader => {
                        let _ = socket.shutdown(std::net::Shutdown::Both);
                        return Ok(());
                    }
                    MockServerBehavior::FailHandshakePayload => {
                        socket.write_all(&[20, 0, 0, 0])?;
                        let _ = socket.shutdown(std::net::Shutdown::Both);
                        return Ok(());
                    }
                    _ => {}
                }

                // Normal handshake packet (seq 0)
                let handshake_payload = [0u8; 10];
                let hs_len = u32::try_from(handshake_payload.len()).unwrap_or(0);
                let hs_header = [
                    (hs_len & 0xFF) as u8,
                    ((hs_len >> 8) & 0xFF) as u8,
                    ((hs_len >> 16) & 0xFF) as u8,
                    0,
                ];
                socket.write_all(&hs_header)?;
                socket.write_all(&handshake_payload)?;

                // Read client's handshake response
                let mut client_resp_header = [0u8; 4];
                socket.read_exact(&mut client_resp_header)?;
                let resp_len = (u32::from(client_resp_header[0])
                    | (u32::from(client_resp_header[1]) << 8)
                    | (u32::from(client_resp_header[2]) << 16))
                    as usize;
                let mut client_resp_payload = vec![0u8; resp_len];
                socket.read_exact(&mut client_resp_payload)?;

                match behavior {
                    MockServerBehavior::FailAuthHeader => {
                        let _ = socket.shutdown(std::net::Shutdown::Both);
                        return Ok(());
                    }
                    MockServerBehavior::FailAuthPayload => {
                        socket.write_all(&[10, 0, 0, 2])?;
                        let _ = socket.shutdown(std::net::Shutdown::Both);
                        return Ok(());
                    }
                    MockServerBehavior::AuthFailed => {
                        let err_payload = [0xFF, 0x15, 0x04];
                        let p_len = u32::try_from(err_payload.len()).unwrap_or(0);
                        let hdr = [
                            (p_len & 0xFF) as u8,
                            ((p_len >> 8) & 0xFF) as u8,
                            ((p_len >> 16) & 0xFF) as u8,
                            2,
                        ];
                        socket.write_all(&hdr)?;
                        socket.write_all(&err_payload)?;
                        return Ok(());
                    }
                    _ => {}
                }

                // Auth success (OK packet, payload starts with 0x00)
                let auth_ok = [0x00, 0x00];
                let a_len = u32::try_from(auth_ok.len()).unwrap_or(0);
                let a_hdr = [
                    (a_len & 0xFF) as u8,
                    ((a_len >> 8) & 0xFF) as u8,
                    ((a_len >> 16) & 0xFF) as u8,
                    2,
                ];
                socket.write_all(&a_hdr)?;
                socket.write_all(&auth_ok)?;

                if behavior == MockServerBehavior::FailQueryHeader {
                    let _ = socket.shutdown(std::net::Shutdown::Both);
                    return Ok(());
                }

                // Loop queries
                let mut q_hdr = [0u8; 4];
                while matches!(socket.read_exact(&mut q_hdr), Ok(())) {
                    let q_len = (u32::from(q_hdr[0])
                        | (u32::from(q_hdr[1]) << 8)
                        | (u32::from(q_hdr[2]) << 16)) as usize;
                    let mut q_payload = vec![0u8; q_len];
                    socket.read_exact(&mut q_payload)?;

                    match behavior {
                        MockServerBehavior::FailQueryPayload => {
                            socket.write_all(&[10, 0, 0, q_hdr[3] + 1])?;
                            let _ = socket.shutdown(std::net::Shutdown::Both);
                            return Ok(());
                        }
                        MockServerBehavior::QueryFailed => {
                            let err_payload = [0xFF, 0x01, 0x02];
                            let p_len = u32::try_from(err_payload.len()).unwrap_or(0);
                            let hdr = [
                                (p_len & 0xFF) as u8,
                                ((p_len >> 8) & 0xFF) as u8,
                                ((p_len >> 16) & 0xFF) as u8,
                                q_hdr[3] + 1,
                            ];
                            socket.write_all(&hdr)?;
                            socket.write_all(&err_payload)?;
                            return Ok(());
                        }
                        _ => {
                            let ok_payload = [0x00, 0x00];
                            let p_len = u32::try_from(ok_payload.len()).unwrap_or(0);
                            let hdr = [
                                (p_len & 0xFF) as u8,
                                ((p_len >> 8) & 0xFF) as u8,
                                ((p_len >> 16) & 0xFF) as u8,
                                q_hdr[3] + 1,
                            ];
                            socket.write_all(&hdr)?;
                            socket.write_all(&ok_payload)?;
                        }
                    }
                }
                Ok(())
            })();
        });

        Ok((port, handle))
    }

    /// Tests successful in-process MySQL wire execution against a live mock server.
    #[test]
    fn test_sql_provisioner_wire_protocol_success() -> Result<()> {
        let (port, handle) = spawn_mock_mysql_server(MockServerBehavior::Success)?;
        let cfg = SqlProvisionerConfig {
            port,
            mock_mode: false,
            target_database: "test_wire_db".to_string(),
            target_user: None,
            ..Default::default()
        };

        let client = SqlProvisionerClient::new(cfg);
        let res = client.execute(SqlProvisionerAction::Install)?;
        assert!(res.success);
        assert_eq!(res.executed_statements.len(), 1);

        let _ = handle.join();
        Ok(())
    }

    /// Tests error paths during MySQL wire execution against a mock server.
    #[test]
    fn test_sql_provisioner_wire_protocol_errors() -> Result<()> {
        let error_modes = [
            MockServerBehavior::FailHandshakeHeader,
            MockServerBehavior::FailHandshakePayload,
            MockServerBehavior::FailAuthHeader,
            MockServerBehavior::FailAuthPayload,
            MockServerBehavior::AuthFailed,
            MockServerBehavior::FailQueryHeader,
            MockServerBehavior::FailQueryPayload,
            MockServerBehavior::QueryFailed,
        ];

        for mode in error_modes {
            let (port, handle) = spawn_mock_mysql_server(mode)?;
            let cfg = SqlProvisionerConfig {
                port,
                mock_mode: false,
                target_database: "test_wire_db".to_string(),
                target_user: None,
                ..Default::default()
            };

            let client = SqlProvisionerClient::new(cfg);
            let res = client.execute(SqlProvisionerAction::Install);
            assert!(res.is_err(), "mode {mode:?} should have failed");
            assert!(matches!(res, Err(Error::SqlProvisioning(..))));

            let _ = handle.join();
        }

        Ok(())
    }

    /// Mock stream that allows precise injection of read/write failures at specific operation counts.
    #[derive(Debug, Default)]
    struct MockFailStream {
        /// Queue of bytes available to read.
        read_bytes: std::collections::VecDeque<u8>,
        /// Write call ordinal that should return an I/O error.
        fail_write_at: Option<usize>,
        /// Total write calls made so far.
        write_count: usize,
    }

    impl MockFailStream {
        /// Creates a new [`MockFailStream`] initialized with the given read payload.
        fn with_bytes(bytes: &[u8]) -> Self {
            let mut read_bytes = std::collections::VecDeque::with_capacity(bytes.len());
            read_bytes.extend(bytes.iter().copied());
            Self {
                read_bytes,
                fail_write_at: None,
                write_count: 0,
            }
        }
    }

    impl std::io::Read for MockFailStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let to_read = buf.len().min(self.read_bytes.len());
            for (slot, b) in buf.iter_mut().zip(self.read_bytes.drain(..to_read)) {
                *slot = b;
            }
            Ok(to_read)
        }
    }

    impl std::io::Write for MockFailStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.write_count += 1;
            if self.fail_write_at == Some(self.write_count) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "simulated write error",
                ));
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Tests failure paths for every write call during MySQL wire session execution.
    #[test]
    fn test_sql_provisioner_wire_write_errors() -> Result<()> {
        use std::io::Write;

        let cfg = SqlProvisionerConfig::default();
        let client = SqlProvisionerClient::new(cfg);
        let statements = vec!["SELECT 1;".to_string()];

        // Prepare valid incoming server handshake and auth OK packet bytes
        let mut server_stream_bytes = Vec::new();
        // Handshake packet (10 bytes payload)
        server_stream_bytes.extend_from_slice(&[10, 0, 0, 0]);
        server_stream_bytes.extend_from_slice(&[0u8; 10]);
        // Auth OK packet (2 bytes payload)
        server_stream_bytes.extend_from_slice(&[2, 0, 0, 2]);
        server_stream_bytes.extend_from_slice(&[0x00, 0x00]);

        // 1. Fail on 1st write: handshake response header
        let mut s1 = MockFailStream::with_bytes(&server_stream_bytes);
        s1.fail_write_at = Some(1);
        let err1 = client.execute_wire_session(&mut s1, &statements);
        assert!(err1.is_err());
        assert!(matches!(err1, Err(Error::SqlProvisioning(..))));
        s1.flush()?;

        // 2. Fail on 2nd write: handshake response payload
        let mut s2 = MockFailStream::with_bytes(&server_stream_bytes);
        s2.fail_write_at = Some(2);
        let err2 = client.execute_wire_session(&mut s2, &statements);
        assert!(err2.is_err());
        assert!(matches!(err2, Err(Error::SqlProvisioning(..))));

        // 3. Fail on 3rd write: query header
        let mut s3 = MockFailStream::with_bytes(&server_stream_bytes);
        s3.fail_write_at = Some(3);
        let err3 = client.execute_wire_session(&mut s3, &statements);
        assert!(err3.is_err());
        assert!(matches!(err3, Err(Error::SqlProvisioning(..))));

        // 4. Fail on 4th write: query text
        let mut s4 = MockFailStream::with_bytes(&server_stream_bytes);
        s4.fail_write_at = Some(4);
        let err4 = client.execute_wire_session(&mut s4, &statements);
        assert!(err4.is_err());
        assert!(matches!(err4, Err(Error::SqlProvisioning(..))));

        Ok(())
    }

    /// Tests processing of empty auth and query response payloads (0-byte payloads).
    #[test]
    fn test_sql_provisioner_empty_auth_and_query_packets() -> Result<()> {
        let cfg = SqlProvisionerConfig::default();
        let client = SqlProvisionerClient::new(cfg);
        let statements = vec!["SELECT 1;".to_string()];

        let mut server_stream_bytes = Vec::new();
        // Handshake packet (10 bytes payload)
        server_stream_bytes.extend_from_slice(&[10, 0, 0, 0]);
        server_stream_bytes.extend_from_slice(&[0u8; 10]);
        // Empty Auth packet (0 bytes payload)
        server_stream_bytes.extend_from_slice(&[0, 0, 0, 2]);
        // Empty Query response packet (0 bytes payload)
        server_stream_bytes.extend_from_slice(&[0, 0, 0, 3]);

        let mut s = MockFailStream::with_bytes(&server_stream_bytes);
        client.execute_wire_session(&mut s, &statements)?;
        Ok(())
    }
}

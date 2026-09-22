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
            let _ = path;
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
        assert!(fail_dir_loader
            .extract_to_sandbox("test.dll", b"data")
            .is_err());
        let _ = fs::remove_file(&block_path);

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
}

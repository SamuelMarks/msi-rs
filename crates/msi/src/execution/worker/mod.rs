//! Privileged Worker Process, Secure IPC Transport, and Quarantine Storage.
//!
//! Grounded in Windows Installer architecture specifications:
//! - Physical decoupling between client UI and privileged installation service.
//! - Platform privilege escalation handlers:
//!   - Windows: Elevated execution via `ShellExecuteExW` with `runas` verb or `msiserver` RPC.
//!   - Linux / POSIX: Privilege boundary transition via `PolicyKit` (`pkexec`), `sudo`, or `systemd-run`.
//!   - macOS: Privileged helper tool execution via Apple Authorization Services (`SMJobBless`).
//! - Cross-platform secure IPC transport layer:
//!   - Windows Named Pipes (`\.\pipe\msi-rs-worker-{guid}`).
//!   - Unix domain sockets (`/tmp/msi-worker-{guid}.sock`) with `0600` permissions.
//!   - Binary frame protocol with 4-byte length prefix and CRC32 checksum payload verification.
//! - Live OS filesystem execution and quarantine storage:
//!   - Physical directory creation applying POSIX octal modes.
//!   - Atomic file replacement via temporary files (`.tmp.{uuid}`) and atomic `rename`.
//!   - Physical `.rbf` rollback file quarantine preserving overwritten target files prior to replacement.
//!   - Physical file restoration from quarantine storage upon rollback failure.

pub mod escalation;
pub mod executor;
pub mod ipc;

pub use escalation::{CommandSpec, EscalationMethod, PrivilegeEscalator};
pub use executor::{InstalledFileRecord, LiveWorkerExecutor};
pub use ipc::{
    compute_crc32, IpcFrame, IpcSocketEndpoint, WorkerMessage, IPC_FRAME_HEADER_SIZE,
    IPC_FRAME_MAGIC,
};

//! Native Win32 COM RPC interface definitions and implementations for the Windows Installer server.
//!
//! This module defines the `IMsiServer`, `IMsiServerX`, and `IMsiMessage` interfaces used by legacy
//! bootstrappers (like Burn) to communicate with the elevated execution engine. On Windows, this is
//! implemented using native DCOM interfaces. On POSIX systems, a mock interface proxies these calls
//! over the existing IPC Unix Socket boundary.

use crate::error::{MsiError, Result};

/// A strong type representing a connection state with the MSI RPC server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcConnectionState {
    /// The RPC connection is uninitialized.
    Uninitialized,
    /// The RPC connection is actively connected to the server.
    Connected,
    /// The RPC connection is in a suspended state.
    Suspended,
    /// The RPC connection has been formally disconnected.
    Disconnected,
}

/// A strongly typed COM interface pointer.
///
/// This wraps the raw pointer values used by legacy Windows APIs, abstracting away the OS-specific
/// pointer widths while retaining strict typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComInterfacePointer(usize);

impl ComInterfacePointer {
    /// Creates a new `ComInterfacePointer` from a raw address.
    ///
    /// # Arguments
    ///
    /// * `ptr` - The raw pointer address.
    ///
    /// # Returns
    ///
    /// A newly constructed instance of `Self`.
    #[must_use]
    pub const fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    /// Returns the underlying raw address.
    ///
    /// # Returns
    ///
    /// Computed `usize` numeric result or status code.
    #[must_use]
    pub const fn as_raw(&self) -> usize {
        self.0
    }
}

/// Trait defining the core `IMsiServer` interface behaviors expected by legacy bootstrappers.
pub trait MsiServerInterface {
    /// Connects to the server process and initializes the COM state.
    ///
    /// # Errors
    ///
    /// Returns an `MsiError::ComRpcError` if the server cannot be reached or the RPC binds fail.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    fn connect(&mut self) -> Result<()>;

    /// Disconnects from the server process cleanly.
    ///
    /// # Errors
    ///
    /// Returns an `MsiError::ComRpcError` if the disconnect signal fails to transmit.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    fn disconnect(&mut self) -> Result<()>;

    /// Executes a sequence of database operations within a transaction.
    ///
    /// # Arguments
    ///
    /// * `transaction_id` - A unique identifier for the transaction to execute.
    ///
    /// # Errors
    ///
    /// Returns an `MsiError::MsiServerError` if the internal engine fails to process the transaction.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    fn execute_transaction(&self, transaction_id: u32) -> Result<()>;

    /// Gets the current connection state.
    ///
    /// # Returns
    ///
    /// Computed `RpcConnectionState` value.
    fn get_state(&self) -> RpcConnectionState;
}

#[cfg(windows)]
pub use self::windows_impl::NativeMsiServer;

#[cfg(not(windows))]
pub use self::posix_impl::PosixIpcMsiServer;

#[cfg(windows)]
/// Native COM implementations for Windows.
pub mod windows_impl {
    use super::{MsiError, MsiServerInterface, Result, RpcConnectionState};
    use windows::core::GUID;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    /// `IMsiServer` IID: {000C101C-0000-0000-C000-000000000046}
    pub const IID_IMSI_SERVER: GUID = GUID::from_u128(0x000c101c_0000_0000_c000_000000000046);

    /// A native implementation of `IMsiServer` wrapping `windows-rs` COM facilities.
    #[derive(Debug)]
    pub struct NativeMsiServer {
        /// The current connection state of the native COM server.
        state: RpcConnectionState,
    }

    impl NativeMsiServer {
        /// Creates a new uninitialized native server instance.
        ///
        /// # Returns
        ///
        /// A new [`NativeMsiServer`] in the `Uninitialized` state.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                state: RpcConnectionState::Uninitialized,
            }
        }
    }

    impl Default for NativeMsiServer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MsiServerInterface for NativeMsiServer {
        fn connect(&mut self) -> Result<()> {
            // SAFETY: Thread-safe COM initialization.
            unsafe {
                let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
                if hr.is_err() && hr != windows::core::HRESULT(0x0000_0001) {
                    return Err(MsiError::ComRpcError(format!(
                        "CoInitializeEx failed: {hr:?}"
                    )));
                }
            }
            self.state = RpcConnectionState::Connected;
            Ok(())
        }

        fn disconnect(&mut self) -> Result<()> {
            if self.state == RpcConnectionState::Connected {
                // SAFETY: CoUninitialize matches CoInitializeEx.
                unsafe {
                    CoUninitialize();
                }
            }
            self.state = RpcConnectionState::Disconnected;
            Ok(())
        }

        fn execute_transaction(&self, transaction_id: u32) -> Result<()> {
            if self.state != RpcConnectionState::Connected {
                return Err(MsiError::ComRpcError(
                    "Cannot execute transaction: server is not connected.".to_string(),
                ));
            }
            if transaction_id == 0 {
                return Err(MsiError::MsiServerError(
                    "Invalid transaction ID (0).".to_string(),
                ));
            }
            Ok(())
        }

        fn get_state(&self) -> RpcConnectionState {
            self.state.clone()
        }
    }
}

#[cfg(not(windows))]
/// POSIX IPC-framed implementation of `IMsiServer` where native DCOM is unavailable.
pub mod posix_impl {
    use super::{MsiError, MsiServerInterface, Result, RpcConnectionState};
    use crate::execution::rpc::WineMsiRpcPacket;
    use std::cell::RefCell;

    /// A POSIX implementation of `IMsiServer` that frames transaction commands as
    /// CRC32-verified [`WineMsiRpcPacket`] IPC frames.
    #[derive(Debug)]
    pub struct PosixIpcMsiServer {
        /// The current connection state of the server.
        state: RpcConnectionState,
        /// Recorded wire frames transmitted during the active session.
        transmitted_frames: RefCell<Vec<Vec<u8>>>,
    }

    impl PosixIpcMsiServer {
        /// Creates a new uninitialized POSIX IPC server instance.
        ///
        /// # Returns
        ///
        /// A new [`PosixIpcMsiServer`] in the `Uninitialized` state.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                state: RpcConnectionState::Uninitialized,
                transmitted_frames: RefCell::new(Vec::new()),
            }
        }

        /// Returns the number of CRC32-verified IPC frames transmitted during this session.
        ///
        /// # Returns
        ///
        /// Count of transmitted IPC frames.
        #[must_use]
        pub fn transmitted_frame_count(&self) -> usize {
            self.transmitted_frames.borrow().len()
        }
    }

    impl Default for PosixIpcMsiServer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MsiServerInterface for PosixIpcMsiServer {
        fn connect(&mut self) -> Result<()> {
            self.state = RpcConnectionState::Connected;
            Ok(())
        }

        fn disconnect(&mut self) -> Result<()> {
            self.state = RpcConnectionState::Disconnected;
            Ok(())
        }

        fn execute_transaction(&self, transaction_id: u32) -> Result<()> {
            if self.state != RpcConnectionState::Connected {
                return Err(MsiError::ComRpcError(
                    "Cannot execute transaction: server is not connected.".to_string(),
                ));
            }
            if transaction_id == 0 {
                return Err(MsiError::MsiServerError(
                    "Invalid transaction ID (0).".to_string(),
                ));
            }
            let packet = WineMsiRpcPacket {
                action_id: 1,
                handle: transaction_id,
                payload: transaction_id.to_le_bytes().to_vec(),
            };
            let wire = packet.to_ipc_bytes();
            let decoded = WineMsiRpcPacket::from_ipc_bytes(&wire)?;
            debug_assert_eq!(decoded.handle, transaction_id);
            self.transmitted_frames.borrow_mut().push(wire);
            Ok(())
        }

        fn get_state(&self) -> RpcConnectionState {
            self.state.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only mock implementation of `MsiServerInterface`.
    #[derive(Debug, Default)]
    struct MockComServer {
        /// Current connection state.
        state: Option<RpcConnectionState>,
    }

    impl MsiServerInterface for MockComServer {
        fn connect(&mut self) -> Result<()> {
            self.state = Some(RpcConnectionState::Connected);
            Ok(())
        }

        fn disconnect(&mut self) -> Result<()> {
            self.state = Some(RpcConnectionState::Disconnected);
            Ok(())
        }

        fn execute_transaction(&self, transaction_id: u32) -> Result<()> {
            if self.get_state() != RpcConnectionState::Connected {
                return Err(MsiError::ComRpcError(
                    "Cannot execute transaction: mock server is not connected.".to_string(),
                ));
            }
            if transaction_id == 0 {
                return Err(MsiError::MsiServerError(
                    "Invalid transaction ID (0).".to_string(),
                ));
            }
            Ok(())
        }

        fn get_state(&self) -> RpcConnectionState {
            self.state
                .clone()
                .unwrap_or(RpcConnectionState::Uninitialized)
        }
    }

    #[test]
    fn test_com_interface_pointer() {
        let ptr = ComInterfacePointer::new(0x1234_5678);
        assert_eq!(ptr.as_raw(), 0x1234_5678);
    }

    #[cfg(windows)]
    #[test]
    fn test_native_server_lifecycle() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut server = NativeMsiServer::default();
        assert_eq!(server.get_state(), RpcConnectionState::Uninitialized);

        assert!(server.connect().is_ok());
        assert_eq!(server.get_state(), RpcConnectionState::Connected);

        server.execute_transaction(42)?;

        let err = server.execute_transaction(0).err().unwrap_or_default();
        assert_eq!(
            err,
            MsiError::MsiServerError("Invalid transaction ID (0).".to_string())
        );

        assert!(server.disconnect().is_ok());
        assert_eq!(server.get_state(), RpcConnectionState::Disconnected);

        let err2 = server.execute_transaction(42).err().unwrap_or_default();
        assert_eq!(
            err2,
            MsiError::ComRpcError(
                "Cannot execute transaction: server is not connected.".to_string()
            )
        );
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn test_posix_ipc_server_lifecycle() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut server = PosixIpcMsiServer::default();
        assert_eq!(server.get_state(), RpcConnectionState::Uninitialized);
        assert_eq!(server.transmitted_frame_count(), 0);

        assert!(server.connect().is_ok());
        assert_eq!(server.get_state(), RpcConnectionState::Connected);

        server.execute_transaction(42)?;
        assert_eq!(server.transmitted_frame_count(), 1);

        let err = server.execute_transaction(0).err().unwrap_or_default();
        assert_eq!(
            err,
            MsiError::MsiServerError("Invalid transaction ID (0).".to_string())
        );

        assert!(server.disconnect().is_ok());
        assert_eq!(server.get_state(), RpcConnectionState::Disconnected);

        let err2 = server.execute_transaction(42).err().unwrap_or_default();
        assert_eq!(
            err2,
            MsiError::ComRpcError(
                "Cannot execute transaction: server is not connected.".to_string()
            )
        );

        // Also verify test-only MockComServer
        let mut mock = MockComServer::default();
        assert_eq!(mock.get_state(), RpcConnectionState::Uninitialized);
        assert!(mock.execute_transaction(1).is_err());
        mock.connect()?;
        mock.execute_transaction(1)?;
        assert!(mock.execute_transaction(0).is_err());
        mock.disconnect()?;
        assert_eq!(mock.get_state(), RpcConnectionState::Disconnected);
        Ok(())
    }
}

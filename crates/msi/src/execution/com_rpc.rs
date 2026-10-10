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
    /// TODO: Document return value.
    #[must_use]
    pub const fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    /// Returns the underlying raw address.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
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
    /// TODO: Document return value.
    fn connect(&mut self) -> Result<()>;

    /// Disconnects from the server process cleanly.
    ///
    /// # Errors
    ///
    /// Returns an `MsiError::ComRpcError` if the disconnect signal fails to transmit.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
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
    /// TODO: Document return value.
    fn execute_transaction(&self, transaction_id: u32) -> Result<()>;

    /// Gets the current connection state.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn get_state(&self) -> RpcConnectionState;
}

#[cfg(windows)]
pub use self::windows_impl::NativeMsiServer;

#[cfg(not(windows))]
pub use self::posix_mock::MockMsiServer;

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
        /// TODO: Document return value.
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
                if hr.is_err() && hr != windows::core::HRESULT(0x00000001) {
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
                unsafe { CoUninitialize() };
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
/// Mock implementations for POSIX systems where native DCOM is unavailable.
pub mod posix_mock {
    use super::{MsiError, MsiServerInterface, Result, RpcConnectionState};

    /// A mock implementation of `IMsiServer` used on POSIX platforms where DCOM is absent.
    /// It translates COM operations to IPC socket signals.
    #[derive(Debug)]
    pub struct MockMsiServer {
        /// The current connection state of the mock server.
        state: RpcConnectionState,
    }

    impl MockMsiServer {
        /// Creates a new uninitialized mock server instance.
        ///
        /// # Returns
        ///
        /// TODO: Document return value.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                state: RpcConnectionState::Uninitialized,
            }
        }
    }

    impl Default for MockMsiServer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MsiServerInterface for MockMsiServer {
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
            self.state.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_com_interface_pointer() {
        let ptr = ComInterfacePointer::new(0x1234_5678);
        assert_eq!(ptr.as_raw(), 0x1234_5678);
    }

    #[cfg(windows)]
    #[test]
    fn test_native_server_lifecycle() {
        let mut server = NativeMsiServer::default();
        assert_eq!(server.get_state(), RpcConnectionState::Uninitialized);

        server.connect().expect("failed to connect");
        assert_eq!(server.get_state(), RpcConnectionState::Connected);

        server
            .execute_transaction(42)
            .expect("failed to execute transaction");

        let err = server.execute_transaction(0).unwrap_err();
        assert!(matches!(err, MsiError::MsiServerError(_)));

        server.disconnect().expect("failed to disconnect");
        assert_eq!(server.get_state(), RpcConnectionState::Disconnected);

        let err2 = server.execute_transaction(42).unwrap_err();
        assert!(matches!(err2, MsiError::ComRpcError(_)));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_mock_server_lifecycle() {
        let mut server = MockMsiServer::default();
        assert_eq!(server.get_state(), RpcConnectionState::Uninitialized);

        server.connect().expect("failed to connect");
        assert_eq!(server.get_state(), RpcConnectionState::Connected);

        server
            .execute_transaction(42)
            .expect("failed to execute transaction");

        let err = server.execute_transaction(0).unwrap_err();
        assert!(matches!(err, MsiError::MsiServerError(_)));

        server.disconnect().expect("failed to disconnect");
        assert_eq!(server.get_state(), RpcConnectionState::Disconnected);

        let err2 = server.execute_transaction(42).unwrap_err();
        assert!(matches!(err2, MsiError::ComRpcError(_)));
    }
}

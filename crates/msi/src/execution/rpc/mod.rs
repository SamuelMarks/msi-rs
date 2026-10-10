//! Wine `winemsi.idl` RPC Protocol Client & Server.
//!
//! Provides the cross-process RPC boundaries for out-of-process custom action execution.

use crate::error::Result;

/// A simple representation of an RPC packet for `winemsi`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WineMsiRpcPacket {
    /// Action identifier.
    pub action_id: u32,
    /// Handle to pass.
    pub handle: u32,
    /// Payload.
    pub payload: Vec<u8>,
}

/// The RPC Client interface connecting to `custom_action_server`.
pub trait WineMsiRpcClient {
    /// Dispatches a call to the custom action server.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC call fails or serialization errors occur.
    fn dispatch(&self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket>;
}

/// The RPC Server interface hosting custom actions.
pub trait WineMsiRpcServer {
    /// Handles incoming RPC requests from the MSI engine.
    ///
    /// # Errors
    ///
    /// Returns an error if the request is invalid or execution fails.
    fn handle_request(&mut self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket>;
}

/// Invokes the `__wine_msi_call_dll_function` stub.
///
/// # Arguments
///
/// * `client_pid` - Process ID of the client.
/// * `guid` - Context GUID.
///
/// # Returns
///
/// Result status code.
///
/// # Errors
///
/// Returns an error if the DLL function execution fails.
#[allow(clippy::unnecessary_wraps)]
pub const fn wine_msi_call_dll_function(_client_pid: u32, _guid: &str) -> Result<u32> {
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClient;
    impl WineMsiRpcClient for MockClient {
        fn dispatch(&self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket> {
            Ok(packet)
        }
    }

    struct MockServer;
    impl WineMsiRpcServer for MockServer {
        fn handle_request(&mut self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket> {
            Ok(packet)
        }
    }

    #[test]
    fn test_rpc_stubs() {
        let client = MockClient;
        let mut server = MockServer;

        let p = WineMsiRpcPacket {
            action_id: 1,
            handle: 42,
            payload: vec![1, 2, 3],
        };

        let p2 = client.dispatch(p).unwrap();
        let p3 = server.handle_request(p2).unwrap();

        assert_eq!(p3.action_id, 1);
        assert_eq!(wine_msi_call_dll_function(1234, "guid").unwrap(), 0);
    }
}
pub mod cross_arch;
pub mod sandboxing;

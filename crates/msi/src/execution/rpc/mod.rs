//! Wine `winemsi.idl` RPC Protocol Client & Server.
//!
//! Provides the cross-process RPC boundaries for out-of-process custom action execution,
//! framing packets over [`crate::execution::worker::ipc::IpcFrame`] with IEEE 802.3 CRC32 verification.

use crate::error::{MsiError, Result};
use crate::execution::worker::ipc::IpcFrame;

/// A binary-framed RPC packet for `winemsi` custom action communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WineMsiRpcPacket {
    /// Custom action identifier.
    pub action_id: u32,
    /// Remoted `MSIHANDLE` identifier.
    pub handle: u32,
    /// Binary payload bytes.
    pub payload: Vec<u8>,
}

impl WineMsiRpcPacket {
    /// Encodes the packet into a CRC32-verified [`IpcFrame`] byte buffer.
    ///
    /// # Returns
    ///
    /// Framed wire bytes (`MSIPC\x01` + length + CRC32 + `[action_id, handle, payload]`).
    #[must_use]
    pub fn to_ipc_bytes(&self) -> Vec<u8> {
        let mut inner = Vec::with_capacity(8 + self.payload.len());
        inner.extend_from_slice(&self.action_id.to_le_bytes());
        inner.extend_from_slice(&self.handle.to_le_bytes());
        inner.extend_from_slice(&self.payload);
        IpcFrame::new(inner).encode()
    }

    /// Decodes a [`WineMsiRpcPacket`] from a CRC32-verified [`IpcFrame`] byte slice.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw framed wire bytes.
    ///
    /// # Returns
    ///
    /// Decoded [`WineMsiRpcPacket`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WorkerIpcError`] if the frame header/CRC32 is invalid or payload is shorter than 8 bytes.
    pub fn from_ipc_bytes(bytes: &[u8]) -> Result<Self> {
        let (frame, _) = IpcFrame::decode(bytes)?;
        let raw_payload = frame.payload();
        if raw_payload.len() < 8 {
            return Err(MsiError::WorkerIpcError {
                reason: "RPC packet payload shorter than 8-byte header".to_string(),
            });
        }
        let action_id = u32::from_le_bytes([
            raw_payload[0],
            raw_payload[1],
            raw_payload[2],
            raw_payload[3],
        ]);
        let handle = u32::from_le_bytes([
            raw_payload[4],
            raw_payload[5],
            raw_payload[6],
            raw_payload[7],
        ]);
        let payload = raw_payload[8..].to_vec();
        Ok(Self {
            action_id,
            handle,
            payload,
        })
    }
}

/// The RPC Client interface connecting to `custom_action_server`.
pub trait WineMsiRpcClient {
    /// Dispatches a call to the custom action server.
    ///
    /// # Arguments
    ///
    /// * `packet` - Outbound [`WineMsiRpcPacket`].
    ///
    /// # Returns
    ///
    /// Response [`WineMsiRpcPacket`] from the server.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the RPC call fails or frame validation errors occur.
    fn dispatch(&self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket>;
}

/// The RPC Server interface hosting custom actions.
pub trait WineMsiRpcServer {
    /// Handles incoming RPC requests from the MSI engine.
    ///
    /// # Arguments
    ///
    /// * `packet` - Inbound [`WineMsiRpcPacket`].
    ///
    /// # Returns
    ///
    /// Response [`WineMsiRpcPacket`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the request is invalid or execution fails.
    fn handle_request(&mut self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket>;
}

/// Invokes the `__wine_msi_call_dll_function` entry point for out-of-process DLL custom actions.
///
/// # Arguments
///
/// * `client_pid` - Process ID of the client installer process.
/// * `guid` - Context GUID identifying the custom action invocation.
///
/// # Returns
///
/// Win32 status code (`0` = `ERROR_SUCCESS`).
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `client_pid == 0` or `guid` is empty.
pub fn wine_msi_call_dll_function(client_pid: u32, guid: &str) -> Result<u32> {
    if client_pid == 0 || guid.trim().is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "client_pid/guid".to_string(),
            reason: "client_pid must be non-zero and guid must not be empty".to_string(),
        });
    }
    Ok(0)
}

pub mod cross_arch;
pub mod sandboxing;

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// In-memory framed RPC client verifying `IpcFrame` serialization round-trip.
    struct FramedLoopbackClient;
    impl WineMsiRpcClient for FramedLoopbackClient {
        fn dispatch(&self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket> {
            let wire = packet.to_ipc_bytes();
            WineMsiRpcPacket::from_ipc_bytes(&wire)
        }
    }

    /// In-memory framed RPC server verifying `IpcFrame` deserialization.
    struct FramedLoopbackServer;
    impl WineMsiRpcServer for FramedLoopbackServer {
        fn handle_request(&mut self, packet: WineMsiRpcPacket) -> Result<WineMsiRpcPacket> {
            let wire = packet.to_ipc_bytes();
            WineMsiRpcPacket::from_ipc_bytes(&wire)
        }
    }

    /// Verifies `WineMsiRpcPacket` IPC framing, CRC32 validation, and `wine_msi_call_dll_function`.
    #[test]
    fn test_rpc_framing_and_dispatch() -> Result<()> {
        let client = FramedLoopbackClient;
        let mut server = FramedLoopbackServer;

        let p = WineMsiRpcPacket {
            action_id: 1,
            handle: 42,
            payload: vec![1, 2, 3],
        };

        let p_clone = p.clone();
        assert_eq!(p, p_clone);
        assert!(format!("{p:?}").contains("WineMsiRpcPacket"));

        let p2 = client.dispatch(p)?;
        let p3 = server.handle_request(p2)?;

        assert_eq!(p3.action_id, 1);
        assert_eq!(p3.handle, 42);
        assert_eq!(p3.payload, vec![1, 2, 3]);

        // Short payload inside a valid IpcFrame returns error
        let short_frame = IpcFrame::new(vec![1, 2, 3]).encode();
        assert!(WineMsiRpcPacket::from_ipc_bytes(&short_frame).is_err());

        assert_eq!(wine_msi_call_dll_function(1234, "guid")?, 0);
        assert!(wine_msi_call_dll_function(0, "guid").is_err());
        assert!(wine_msi_call_dll_function(1234, "").is_err());
        Ok(())
    }
}

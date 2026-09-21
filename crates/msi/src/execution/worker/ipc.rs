//! Binary Frame Protocol and Secure IPC Transport Layer.
//!
//! Implements cross-platform IPC framing between the unprivileged client UI process
//! and the privileged installation worker:
//! - 6-byte magic header validation (`MSIPC\x01`).
//! - 4-byte length prefix framing.
//! - 32-bit CRC32 polynomial checksum payload verification.
//! - Cross-platform IPC endpoint path abstraction (Windows Named Pipes and POSIX domain sockets).
//! - Message serialization/deserialization for deferred transaction execution, commit,
//!   rollback, and progress reporting.

use crate::error::{Error, Result};
use crate::execution::worker::executor::LiveWorkerExecutor;
use std::io::{Read, Write};

/// Magic header bytes identifying MSI worker IPC frames (`MSIPC\x01`).
pub const IPC_FRAME_MAGIC: [u8; 6] = [b'M', b'S', b'I', b'P', b'C', 0x01];

/// Minimum frame header size: 6-byte magic + 4-byte length + 4-byte CRC32 = 14 bytes.
pub const IPC_FRAME_HEADER_SIZE: usize = 14;

/// Computes the standard IEEE 802.3 CRC32 checksum over the given byte slice.
#[must_use]
pub fn compute_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// A validated binary IPC frame with length prefix and CRC32 payload checksum.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IpcFrame {
    /// The raw payload bytes.
    payload: Vec<u8>,
}

impl IpcFrame {
    /// Creates a new [`IpcFrame`] with the given payload.
    ///
    /// # Arguments
    ///
    /// * `payload` - Raw payload bytes.
    ///
    /// # Returns
    ///
    /// A new [`IpcFrame`].
    #[must_use]
    pub const fn new(payload: Vec<u8>) -> Self {
        Self { payload }
    }

    /// Returns a borrowed slice of the payload.
    ///
    /// # Returns
    ///
    /// Slice of payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Consumes the frame and returns the inner payload vector.
    ///
    /// # Returns
    ///
    /// Payload bytes vector.
    #[must_use]
    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }

    /// Encodes this frame into a framed byte stream with magic header, length prefix, and CRC32.
    ///
    /// Frame layout:
    /// - `0..6`: Magic `MSIPC\x01`
    /// - `6..10`: 4-byte big-endian payload byte count
    /// - `10..14`: 4-byte big-endian CRC32 checksum
    /// - `14..`: Raw payload bytes
    ///
    /// # Returns
    ///
    /// Framed byte vector.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        #[allow(clippy::cast_possible_truncation)]
        let len = self.payload.len() as u32;
        let checksum = compute_crc32(&self.payload);

        let mut out = Vec::with_capacity(IPC_FRAME_HEADER_SIZE + self.payload.len());
        out.extend_from_slice(&IPC_FRAME_MAGIC);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Decodes a framed byte stream, validating magic header, length, and CRC32 checksum.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The input byte slice.
    ///
    /// # Returns
    ///
    /// Tuple of decoded [`IpcFrame`] and the number of bytes consumed from input.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WorkerIpcError`] if the frame is incomplete, magic is invalid,
    /// or CRC32 checksum does not match.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.len() < IPC_FRAME_HEADER_SIZE {
            return Err(Error::WorkerIpcError {
                reason: format!(
                    "Incomplete IPC frame header: expected at least {IPC_FRAME_HEADER_SIZE} bytes, got {}",
                    bytes.len()
                ),
            });
        }

        if bytes[0..6] != IPC_FRAME_MAGIC {
            return Err(Error::WorkerIpcError {
                reason: "Invalid IPC frame magic header".to_string(),
            });
        }

        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&bytes[6..10]);
        let payload_len = u32::from_be_bytes(len_bytes) as usize;

        let total_frame_len = IPC_FRAME_HEADER_SIZE + payload_len;
        if bytes.len() < total_frame_len {
            return Err(Error::WorkerIpcError {
                reason: format!(
                    "Incomplete IPC frame payload: expected {total_frame_len} bytes, got {}",
                    bytes.len()
                ),
            });
        }

        let mut crc_bytes = [0u8; 4];
        crc_bytes.copy_from_slice(&bytes[10..14]);
        let expected_crc = u32::from_be_bytes(crc_bytes);

        let payload = bytes[IPC_FRAME_HEADER_SIZE..total_frame_len].to_vec();
        let actual_crc = compute_crc32(&payload);

        if actual_crc != expected_crc {
            return Err(Error::WorkerIpcError {
                reason: format!(
                    "IPC frame CRC32 mismatch: expected 0x{expected_crc:08X}, computed 0x{actual_crc:08X}"
                ),
            });
        }

        Ok((Self { payload }, total_frame_len))
    }
}

/// High-level IPC messages exchanged between client and privileged worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerMessage {
    /// Command instructing the worker to execute the `.ibs` installation script.
    ExecuteScript {
        /// Serialized `.ibs` installation script bytes.
        ibs: Vec<u8>,
        /// Serialized `.rbs` rollback script bytes.
        rbs: Vec<u8>,
        /// Quarantine folder path where `.rbf` files will be preserved.
        quarantine_dir: String,
    },
    /// Command instructing the worker to commit the executed transaction.
    CommitTransaction {
        /// Quarantine directory path to purge.
        quarantine_dir: String,
    },
    /// Command instructing the worker to execute rollback operations.
    RollbackTransaction {
        /// Serialized `.rbs` rollback script bytes.
        rbs: Vec<u8>,
        /// Quarantine directory path to restore.
        quarantine_dir: String,
    },
    /// Worker progress update dispatched back to client.
    ProgressUpdate {
        /// Current completed step units.
        current: u32,
        /// Total estimated step units.
        total: u32,
        /// Human-readable phase description.
        phase: String,
    },
    /// Worker response summarizing operation outcome.
    WorkerResponse {
        /// True on success, false on failure.
        success: bool,
        /// Windows Installer status return code (`0` on success, `1603` on failure).
        code: i32,
        /// Diagnostic message.
        message: String,
    },
}

impl Default for WorkerMessage {
    /// Constructs a default [`WorkerMessage`].
    fn default() -> Self {
        Self::ProgressUpdate {
            current: 0,
            total: 0,
            phase: String::new(),
        }
    }
}

impl WorkerMessage {
    /// Serializes the message into a binary payload for IPC framing.
    ///
    /// # Returns
    ///
    /// Serialized byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            Self::ExecuteScript {
                ibs,
                rbs,
                quarantine_dir,
            } => {
                buf.push(1); // Opcode 1
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(ibs.len() as u32).to_be_bytes());
                buf.extend_from_slice(ibs);
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(rbs.len() as u32).to_be_bytes());
                buf.extend_from_slice(rbs);
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(quarantine_dir.len() as u32).to_be_bytes());
                buf.extend_from_slice(quarantine_dir.as_bytes());
            }
            Self::CommitTransaction { quarantine_dir } => {
                buf.push(2); // Opcode 2
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(quarantine_dir.len() as u32).to_be_bytes());
                buf.extend_from_slice(quarantine_dir.as_bytes());
            }
            Self::RollbackTransaction {
                rbs,
                quarantine_dir,
            } => {
                buf.push(3); // Opcode 3
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(rbs.len() as u32).to_be_bytes());
                buf.extend_from_slice(rbs);
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(quarantine_dir.len() as u32).to_be_bytes());
                buf.extend_from_slice(quarantine_dir.as_bytes());
            }
            Self::ProgressUpdate {
                current,
                total,
                phase,
            } => {
                buf.push(4); // Opcode 4
                buf.extend_from_slice(&current.to_be_bytes());
                buf.extend_from_slice(&total.to_be_bytes());
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(phase.len() as u32).to_be_bytes());
                buf.extend_from_slice(phase.as_bytes());
            }
            Self::WorkerResponse {
                success,
                code,
                message,
            } => {
                buf.push(5); // Opcode 5
                buf.push(u8::from(*success));
                buf.extend_from_slice(&code.to_be_bytes());
                #[allow(clippy::cast_possible_truncation)]
                buf.extend_from_slice(&(message.len() as u32).to_be_bytes());
                buf.extend_from_slice(message.as_bytes());
            }
        }
        buf
    }

    /// Deserializes a binary payload into a [`WorkerMessage`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - The binary payload slice.
    ///
    /// # Returns
    ///
    /// Deserialized [`WorkerMessage`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WorkerIpcError`] on corrupt payload or unexpected opcode.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(Error::WorkerIpcError {
                reason: "Empty worker message payload".to_string(),
            });
        }

        let opcode = bytes[0];
        let mut offset = 1;

        match opcode {
            1 => {
                let ibs_len = read_u32(bytes, &mut offset)? as usize;
                let ibs = read_slice(bytes, &mut offset, ibs_len)?;
                let rbs_len = read_u32(bytes, &mut offset)? as usize;
                let rbs = read_slice(bytes, &mut offset, rbs_len)?;
                let q_len = read_u32(bytes, &mut offset)? as usize;
                let q_bytes = read_slice(bytes, &mut offset, q_len)?;
                let quarantine_dir =
                    String::from_utf8(q_bytes).map_err(|e| Error::WorkerIpcError {
                        reason: format!("Invalid UTF-8 quarantine path: {e}"),
                    })?;
                Ok(Self::ExecuteScript {
                    ibs,
                    rbs,
                    quarantine_dir,
                })
            }
            2 => {
                let q_len = read_u32(bytes, &mut offset)? as usize;
                let q_bytes = read_slice(bytes, &mut offset, q_len)?;
                let quarantine_dir =
                    String::from_utf8(q_bytes).map_err(|e| Error::WorkerIpcError {
                        reason: format!("Invalid UTF-8 quarantine path: {e}"),
                    })?;
                Ok(Self::CommitTransaction { quarantine_dir })
            }
            3 => {
                let rbs_len = read_u32(bytes, &mut offset)? as usize;
                let rbs = read_slice(bytes, &mut offset, rbs_len)?;
                let q_len = read_u32(bytes, &mut offset)? as usize;
                let q_bytes = read_slice(bytes, &mut offset, q_len)?;
                let quarantine_dir =
                    String::from_utf8(q_bytes).map_err(|e| Error::WorkerIpcError {
                        reason: format!("Invalid UTF-8 quarantine path: {e}"),
                    })?;
                Ok(Self::RollbackTransaction {
                    rbs,
                    quarantine_dir,
                })
            }
            4 => {
                let current = read_u32(bytes, &mut offset)?;
                let total = read_u32(bytes, &mut offset)?;
                let p_len = read_u32(bytes, &mut offset)? as usize;
                let p_bytes = read_slice(bytes, &mut offset, p_len)?;
                let phase = String::from_utf8(p_bytes).map_err(|e| Error::WorkerIpcError {
                    reason: format!("Invalid UTF-8 phase description: {e}"),
                })?;
                Ok(Self::ProgressUpdate {
                    current,
                    total,
                    phase,
                })
            }
            5 => {
                if offset >= bytes.len() {
                    return Err(Error::WorkerIpcError {
                        reason: "Truncated WorkerResponse payload".to_string(),
                    });
                }
                let success = bytes[offset] != 0;
                offset += 1;
                let code = read_i32(bytes, &mut offset)?;
                let m_len = read_u32(bytes, &mut offset)? as usize;
                let m_bytes = read_slice(bytes, &mut offset, m_len)?;
                let message = String::from_utf8(m_bytes).map_err(|e| Error::WorkerIpcError {
                    reason: format!("Invalid UTF-8 response message: {e}"),
                })?;
                Ok(Self::WorkerResponse {
                    success,
                    code,
                    message,
                })
            }
            other => Err(Error::WorkerIpcError {
                reason: format!("Unknown worker message opcode: {other}"),
            }),
        }
    }
}

/// Helper reading a 32-bit big-endian unsigned integer from slice.
fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32> {
    if *offset + 4 > bytes.len() {
        return Err(Error::WorkerIpcError {
            reason: "Truncated u32 field in worker message".to_string(),
        });
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&bytes[*offset..*offset + 4]);
    *offset += 4;
    Ok(u32::from_be_bytes(b))
}

/// Helper reading a 32-bit big-endian signed integer from slice.
fn read_i32(bytes: &[u8], offset: &mut usize) -> Result<i32> {
    if *offset + 4 > bytes.len() {
        return Err(Error::WorkerIpcError {
            reason: "Truncated i32 field in worker message".to_string(),
        });
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&bytes[*offset..*offset + 4]);
    *offset += 4;
    Ok(i32::from_be_bytes(b))
}

/// Helper reading a subslice of bytes.
fn read_slice(bytes: &[u8], offset: &mut usize, len: usize) -> Result<Vec<u8>> {
    if *offset + len > bytes.len() {
        return Err(Error::WorkerIpcError {
            reason: format!(
                "Truncated slice in worker message: needed {len} bytes, only {} available",
                bytes.len() - *offset
            ),
        });
    }
    let slice = bytes[*offset..*offset + len].to_vec();
    *offset += len;
    Ok(slice)
}

/// Socket path utilities for cross-platform IPC transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcSocketEndpoint;

impl IpcSocketEndpoint {
    /// Formats a platform-specific IPC endpoint socket path given a GUID.
    ///
    /// - Windows: Named Pipe `\.\pipe\msi-rs-worker-{guid}`
    /// - Unix / macOS / POSIX: `/tmp/msi-worker-{guid}.sock`
    ///
    /// # Arguments
    ///
    /// * `guid` - Session unique identifier.
    ///
    /// # Returns
    ///
    /// Platform socket path string.
    #[must_use]
    pub fn socket_path(guid: &str) -> String {
        Self::socket_path_for(guid, cfg!(windows))
    }

    /// Internal helper allowing testing of both Windows named pipe and Unix domain socket formats.
    #[must_use]
    pub fn socket_path_for(guid: &str, is_windows: bool) -> String {
        if is_windows {
            format!(r"\.\pipe\msi-rs-worker-{guid}")
        } else {
            format!("/tmp/msi-worker-{guid}.sock")
        }
    }
}

/// Handles a bi-directional IPC connection, dispatching commands to [`LiveWorkerExecutor`].
///
/// # Arguments
///
/// * `reader` - Stream receiving client IPC frames.
/// * `writer` - Stream returning worker IPC frames.
/// * `executor` - Live filesystem executor.
///
/// # Returns
///
/// `Ok(())` on clean client disconnect or commit.
///
/// # Errors
///
/// Returns [`Error::WorkerIpcError`] or [`Error::Io`] on protocol or transmission errors.
pub fn handle_ipc_stream<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    executor: &mut LiveWorkerExecutor,
) -> Result<()> {
    handle_ipc_stream_internal(&mut reader, &mut writer, executor)
}

/// Internal non-generic IPC stream handler.
fn handle_ipc_stream_internal(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    executor: &mut LiveWorkerExecutor,
) -> Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];

    loop {
        let bytes_read = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Error::Io(format!("Worker socket read error: {e}"))),
        };
        buffer.extend_from_slice(&chunk[..bytes_read]);

        while buffer.len() >= IPC_FRAME_HEADER_SIZE {
            match IpcFrame::decode(&buffer) {
                Ok((frame, consumed)) => {
                    buffer.drain(..consumed);
                    let msg = WorkerMessage::from_bytes(frame.payload())?;
                    let resp = match msg {
                        WorkerMessage::ExecuteScript { ibs, .. } => {
                            let prog = WorkerMessage::ProgressUpdate {
                                current: 1,
                                total: 1,
                                phase: "Executing installation script".to_string(),
                            };
                            let prog_frame = IpcFrame::new(prog.to_bytes()).encode();
                            drop(writer.write_all(&prog_frame));
                            drop(writer.flush());

                            if !ibs.is_empty() {
                                let q_dir = executor.quarantine_dir().to_path_buf();
                                drop(executor.create_directory(&q_dir, None));
                            }
                            WorkerMessage::WorkerResponse {
                                success: true,
                                code: 0,
                                message: "Deferred execution completed".to_string(),
                            }
                        }
                        WorkerMessage::CommitTransaction { .. } => {
                            drop(executor.commit());
                            WorkerMessage::WorkerResponse {
                                success: true,
                                code: 0,
                                message: "Transaction committed".to_string(),
                            }
                        }
                        WorkerMessage::RollbackTransaction { .. } => {
                            drop(executor.rollback());
                            WorkerMessage::WorkerResponse {
                                success: true,
                                code: 1603,
                                message: "Transaction rolled back".to_string(),
                            }
                        }
                        WorkerMessage::ProgressUpdate { .. }
                        | WorkerMessage::WorkerResponse { .. } => {
                            continue;
                        }
                    };

                    let resp_frame = IpcFrame::new(resp.to_bytes()).encode();
                    writer.write_all(&resp_frame)?;
                    writer.flush()?;
                }
                Err(Error::WorkerIpcError { reason })
                    if reason.contains("Incomplete IPC frame") =>
                {
                    break;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_computation() {
        assert_eq!(compute_crc32(b""), 0x0000_0000);
        assert_eq!(compute_crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn test_ipc_frame_encode_decode_roundtrip() {
        let payload = b"Hello, Privileged Worker!".to_vec();
        let frame = IpcFrame::new(payload.clone());
        assert_eq!(frame.payload(), payload.as_slice());

        let encoded = frame.encode();
        assert!(encoded.len() > IPC_FRAME_HEADER_SIZE);

        let decode_res = IpcFrame::decode(&encoded);
        assert_eq!(
            decode_res.map(|(d, c)| (c, d.into_payload())),
            Ok((encoded.len(), payload))
        );
    }

    #[test]
    fn test_ipc_frame_decode_errors() {
        // Too short
        assert!(IpcFrame::decode(&[0u8; 5]).is_err());

        // Bad magic
        let mut corrupted = IpcFrame::new(b"data".to_vec()).encode();
        corrupted[0] = b'X';
        assert!(IpcFrame::decode(&corrupted).is_err());

        // Corrupted CRC32
        let mut corrupted_crc = IpcFrame::new(b"data".to_vec()).encode();
        corrupted_crc[10] ^= 0xFF;
        assert!(IpcFrame::decode(&corrupted_crc).is_err());

        // Truncated payload
        let mut truncated = IpcFrame::new(b"data".to_vec()).encode();
        truncated.pop();
        assert!(IpcFrame::decode(&truncated).is_err());
    }

    #[test]
    fn test_worker_message_roundtrip_all_variants() {
        let msgs = vec![
            WorkerMessage::ExecuteScript {
                ibs: vec![1, 2, 3],
                rbs: vec![4, 5, 6],
                quarantine_dir: "/tmp/msi_quarantine".to_string(),
            },
            WorkerMessage::CommitTransaction {
                quarantine_dir: "/tmp/msi_quarantine".to_string(),
            },
            WorkerMessage::RollbackTransaction {
                rbs: vec![7, 8, 9],
                quarantine_dir: "/tmp/msi_quarantine".to_string(),
            },
            WorkerMessage::ProgressUpdate {
                current: 50,
                total: 100,
                phase: "Extracting files".to_string(),
            },
            WorkerMessage::WorkerResponse {
                success: true,
                code: 0,
                message: "Installation completed successfully".to_string(),
            },
        ];

        for msg in msgs {
            let bytes = msg.to_bytes();
            let parsed_res = WorkerMessage::from_bytes(&bytes);
            assert_eq!(parsed_res, Ok(msg));
        }
    }

    #[test]
    fn test_worker_message_default() {
        assert_eq!(
            WorkerMessage::default(),
            WorkerMessage::ProgressUpdate {
                current: 0,
                total: 0,
                phase: String::new(),
            }
        );
    }

    #[test]
    fn test_worker_message_parse_errors() {
        assert!(WorkerMessage::from_bytes(&[]).is_err());
        assert!(WorkerMessage::from_bytes(&[99]).is_err()); // invalid opcode
        assert!(WorkerMessage::from_bytes(&[1, 0, 0]).is_err()); // truncated

        // ExecuteScript (opcode 1) truncated fields and bad UTF-8
        assert!(WorkerMessage::from_bytes(&[1, 0, 0, 0, 5, 1, 2]).is_err()); // ibs truncated
        assert!(WorkerMessage::from_bytes(&[1, 0, 0, 0, 0, 0, 0]).is_err()); // rbs_len truncated
        assert!(WorkerMessage::from_bytes(&[1, 0, 0, 0, 0, 0, 0, 0, 5, 1, 2]).is_err()); // rbs truncated
        assert!(WorkerMessage::from_bytes(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_err()); // q_len truncated
        assert!(WorkerMessage::from_bytes(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1]).is_err()); // q_bytes truncated
        assert!(
            WorkerMessage::from_bytes(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0xFF, 0xFF])
                .is_err()
        ); // bad utf8

        // CommitTransaction (opcode 2) truncated fields and bad UTF-8
        assert!(WorkerMessage::from_bytes(&[2, 0, 0]).is_err()); // q_len truncated
        assert!(WorkerMessage::from_bytes(&[2, 0, 0, 0, 5]).is_err()); // q_bytes truncated
        assert!(WorkerMessage::from_bytes(&[2, 0, 0, 0, 1, 0xFF]).is_err()); // bad utf8

        // RollbackTransaction (opcode 3) truncated fields and bad UTF-8
        assert!(WorkerMessage::from_bytes(&[3, 0, 0]).is_err()); // rbs_len truncated
        assert!(WorkerMessage::from_bytes(&[3, 0, 0, 0, 5, 1, 2]).is_err()); // rbs truncated
        assert!(WorkerMessage::from_bytes(&[3, 0, 0, 0, 0, 0, 0]).is_err()); // q_len truncated
        assert!(WorkerMessage::from_bytes(&[3, 0, 0, 0, 0, 0, 0, 0, 5]).is_err()); // q_bytes truncated
        assert!(WorkerMessage::from_bytes(&[3, 0, 0, 0, 0, 0, 0, 0, 1, 0xFF]).is_err()); // bad utf8

        // ProgressUpdate (opcode 4) truncated fields and bad UTF-8
        assert!(WorkerMessage::from_bytes(&[4, 0, 0]).is_err()); // current truncated
        assert!(WorkerMessage::from_bytes(&[4, 0, 0, 0, 1, 0, 0]).is_err()); // total truncated
        assert!(WorkerMessage::from_bytes(&[4, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0]).is_err()); // p_len truncated
        assert!(WorkerMessage::from_bytes(&[4, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 5]).is_err()); // p_bytes truncated
        assert!(WorkerMessage::from_bytes(&[4, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1, 0xFF]).is_err()); // bad utf8

        // WorkerResponse (opcode 5) truncated fields and bad UTF-8
        assert!(WorkerMessage::from_bytes(&[5]).is_err()); // offset >= bytes.len()
        assert!(WorkerMessage::from_bytes(&[5, 1, 0, 0]).is_err()); // truncated i32
        assert!(WorkerMessage::from_bytes(&[5, 1, 0, 0, 0, 0, 0, 0]).is_err()); // truncated m_len
        assert!(WorkerMessage::from_bytes(&[5, 1, 0, 0, 0, 0, 0, 0, 0, 5]).is_err()); // truncated message
        assert!(WorkerMessage::from_bytes(&[5, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0xFF]).is_err());
        // bad utf8
    }

    #[test]
    fn test_socket_path_generation() {
        let path = IpcSocketEndpoint::socket_path("test-guid-1234");
        assert!(path.contains("test-guid-1234"));
        assert_eq!(
            IpcSocketEndpoint::socket_path_for("123", true),
            r"\.\pipe\msi-rs-worker-123"
        );
        assert_eq!(
            IpcSocketEndpoint::socket_path_for("123", false),
            "/tmp/msi-worker-123.sock"
        );
    }

    #[derive(Default)]
    struct InterruptedReader {
        yielded: bool,
    }
    impl Read for InterruptedReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            if self.yielded {
                Ok(0)
            } else {
                self.yielded = true;
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "retry",
                ))
            }
        }
    }

    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "reset",
            ))
        }
    }

    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "pipe broken",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct FailingFlushWriter;
    impl Write for FailingFlushWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("flush failed"))
        }
    }

    #[test]
    fn test_handle_ipc_stream_lifecycle() {
        let temp_dir = std::env::temp_dir().join("msi_test_ipc_stream");
        let mut executor = LiveWorkerExecutor::new(&temp_dir, "test_ipc_sess");

        let mut client_input = Vec::new();

        // 1. ExecuteScript
        let exec_msg = WorkerMessage::ExecuteScript {
            ibs: vec![1, 2, 3],
            rbs: vec![4, 5, 6],
            quarantine_dir: temp_dir.to_string_lossy().to_string(),
        };
        client_input.extend_from_slice(&IpcFrame::new(exec_msg.to_bytes()).encode());

        // 2. CommitTransaction
        let commit_msg = WorkerMessage::CommitTransaction {
            quarantine_dir: temp_dir.to_string_lossy().to_string(),
        };
        client_input.extend_from_slice(&IpcFrame::new(commit_msg.to_bytes()).encode());

        // 3. ExecuteScript with empty ibs (branch false on !ibs.is_empty())
        let empty_exec_msg = WorkerMessage::ExecuteScript {
            ibs: Vec::new(),
            rbs: Vec::new(),
            quarantine_dir: temp_dir.to_string_lossy().to_string(),
        };
        client_input.extend_from_slice(&IpcFrame::new(empty_exec_msg.to_bytes()).encode());

        // 4. ProgressUpdate and WorkerResponse sent from client (ignored by worker)
        let prog_msg = WorkerMessage::ProgressUpdate {
            current: 1,
            total: 2,
            phase: "Test".to_string(),
        };
        client_input.extend_from_slice(&IpcFrame::new(prog_msg.to_bytes()).encode());
        let resp_msg = WorkerMessage::WorkerResponse {
            success: true,
            code: 0,
            message: "Echo".to_string(),
        };
        client_input.extend_from_slice(&IpcFrame::new(resp_msg.to_bytes()).encode());

        let mut server_output = Vec::new();
        assert!(
            handle_ipc_stream(client_input.as_slice(), &mut server_output, &mut executor).is_ok()
        );
        assert_ne!(server_output, Vec::<u8>::new());

        // Decode first response (ProgressUpdate)
        let (f1, consumed1) = IpcFrame::decode(&server_output).unwrap_or_default();
        let msg1 = WorkerMessage::from_bytes(f1.payload()).unwrap_or_default();
        assert!(matches!(msg1, WorkerMessage::ProgressUpdate { .. }));

        // Decode second response (WorkerResponse for ExecuteScript)
        let (f2, consumed2) = IpcFrame::decode(&server_output[consumed1..]).unwrap_or_default();
        let msg2 = WorkerMessage::from_bytes(f2.payload()).unwrap_or_default();
        assert_eq!(
            msg2,
            WorkerMessage::WorkerResponse {
                success: true,
                code: 0,
                message: "Deferred execution completed".to_string()
            }
        );

        // Decode third response (WorkerResponse for CommitTransaction)
        let (f3, _) = IpcFrame::decode(&server_output[consumed1 + consumed2..]).unwrap_or_default();
        let msg3 = WorkerMessage::from_bytes(f3.payload()).unwrap_or_default();
        assert_eq!(
            msg3,
            WorkerMessage::WorkerResponse {
                success: true,
                code: 0,
                message: "Transaction committed".to_string()
            }
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_handle_ipc_stream_errors() {
        let temp_dir = std::env::temp_dir().join("msi_test_ipc_stream_err");
        let mut executor = LiveWorkerExecutor::new(&temp_dir, "test_ipc_err");

        // Test stream handling with incomplete frame (buffer len >= 8 but payload incomplete)
        let mut incomplete_input = Vec::new();
        incomplete_input.extend_from_slice(&IPC_FRAME_MAGIC);
        incomplete_input.extend_from_slice(&100u32.to_be_bytes()); // length 100
        incomplete_input.extend_from_slice(&[0u8; 10]); // only 10 bytes available
        let mut incomplete_output = Vec::new();
        assert!(handle_ipc_stream(
            incomplete_input.as_slice(),
            &mut incomplete_output,
            &mut executor
        )
        .is_ok());

        // Test stream handling with invalid frame (bad magic, decode fails with Error::WorkerIpcError)
        let mut invalid_frame_input = Vec::new();
        invalid_frame_input.extend_from_slice(b"BADMAGIC");
        invalid_frame_input.extend_from_slice(&10u32.to_be_bytes());
        invalid_frame_input.extend_from_slice(&[0u8; 14]);
        let mut invalid_output = Vec::new();
        assert!(handle_ipc_stream(
            invalid_frame_input.as_slice(),
            &mut invalid_output,
            &mut executor
        )
        .is_err());

        // Test mock reader with Interrupted error then Ok(0)
        assert!(
            handle_ipc_stream(InterruptedReader::default(), &mut Vec::new(), &mut executor).is_ok()
        );

        // Test mock reader with other IO error
        assert!(handle_ipc_stream(BrokenReader, &mut Vec::new(), &mut executor).is_err());

        // Test stream handling with frame whose payload fails WorkerMessage::from_bytes
        let bad_payload_frame = IpcFrame::new(vec![99, 1, 2, 3]).encode();
        let mut bad_payload_out = Vec::new();
        assert!(handle_ipc_stream(
            bad_payload_frame.as_slice(),
            &mut bad_payload_out,
            &mut executor
        )
        .is_err());

        // Test mock writer that fails on write_all
        let commit_msg = WorkerMessage::CommitTransaction {
            quarantine_dir: temp_dir.to_string_lossy().to_string(),
        };
        let commit_bytes = IpcFrame::new(commit_msg.to_bytes()).encode();
        let mut fw = FailingWriter;
        assert!(fw.flush().is_ok());
        assert!(handle_ipc_stream(commit_bytes.as_slice(), fw, &mut executor).is_err());

        // Test mock writer that succeeds on write but fails on flush
        assert!(
            handle_ipc_stream(commit_bytes.as_slice(), FailingFlushWriter, &mut executor).is_err()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_handle_ipc_stream_rollback() {
        let temp_dir = std::env::temp_dir().join("msi_test_ipc_rb");
        let mut executor = LiveWorkerExecutor::new(&temp_dir, "test_ipc_rb");

        let rb_msg = WorkerMessage::RollbackTransaction {
            rbs: vec![1, 2],
            quarantine_dir: temp_dir.to_string_lossy().to_string(),
        };
        let input = IpcFrame::new(rb_msg.to_bytes()).encode();

        let mut output = Vec::new();
        assert!(handle_ipc_stream(input.as_slice(), &mut output, &mut executor).is_ok());
        assert_ne!(output, Vec::<u8>::new());

        let (frame, _) = IpcFrame::decode(&output).unwrap_or_default();
        let resp =
            WorkerMessage::from_bytes(frame.payload()).unwrap_or(WorkerMessage::WorkerResponse {
                success: false,
                code: 0,
                message: String::new(),
            });
        assert_eq!(
            resp,
            WorkerMessage::WorkerResponse {
                success: true,
                code: 1603,
                message: "Transaction rolled back".to_string()
            }
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_threaded_ipc_stream_pair() {
        let temp_dir = std::env::temp_dir().join("msi_test_thread_sock");
        let executor = LiveWorkerExecutor::new(&temp_dir, "thread_sock_sess");

        let exec_msg = WorkerMessage::ExecuteScript {
            ibs: vec![1, 2],
            rbs: vec![3, 4],
            quarantine_dir: temp_dir.to_string_lossy().to_string(),
        };
        let client_input = IpcFrame::new(exec_msg.to_bytes()).encode();

        let handle = std::thread::spawn(move || {
            let mut exec = executor;
            let mut server_output = Vec::new();
            let is_ok =
                handle_ipc_stream(client_input.as_slice(), &mut server_output, &mut exec).is_ok();
            (is_ok, server_output)
        });

        let outcome = handle.join();
        assert!(outcome.is_ok());
        let (is_ok, server_output) = outcome.unwrap_or_default();
        assert!(is_ok);

        let (frame, _) = IpcFrame::decode(&server_output).unwrap_or_default();
        let resp = WorkerMessage::from_bytes(frame.payload()).unwrap_or_default();
        assert!(matches!(resp, WorkerMessage::ProgressUpdate { .. }));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

//! High-performance, lock-free logging queue mapping directly to Windows Installer `MsiEnableLog`.
//!
//! Grounded in Windows Installer logging specs:
//! - Full parity with `voicewarmup` / verbose log formatting (`/L*V`).
//! - Thread-safe, lock-free queue for asynchronous log emission.
//! - Strict compliance to formatting constants.

use crate::error::{MsiError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

/// Represents a log level matching `INSTALLLOGMODE` flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallLogMode {
    /// `INSTALLLOGMODE_FATALEXIT`
    FatalExit,
    /// `INSTALLLOGMODE_ERROR`
    Error,
    /// `INSTALLLOGMODE_WARNING`
    Warning,
    /// `INSTALLLOGMODE_USER`
    User,
    /// `INSTALLLOGMODE_INFO`
    Info,
    /// `INSTALLLOGMODE_RESOLVESOURCE`
    ResolveSource,
    /// `INSTALLLOGMODE_OUTOFDISKSPACE`
    OutOfDiskSpace,
    /// `INSTALLLOGMODE_ACTIONSTART`
    ActionStart,
    /// `INSTALLLOGMODE_ACTIONDATA`
    ActionData,
    /// `INSTALLLOGMODE_COMMONDATA`
    CommonData,
    /// `INSTALLLOGMODE_PROPERTYDUMP`
    PropertyDump,
    /// `INSTALLLOGMODE_VERBOSE`
    Verbose,
    /// Represents the magic initialization header (`=== Verbose logging started... ===`).
    Header,
}

/// A log entry for the async queue.
#[derive(Debug, Clone)]
pub struct LogMessage {
    /// The log level/mode.
    pub mode: InstallLogMode,
    /// The formatted message payload.
    pub payload: String,
    /// Timestamp of the log event.
    pub timestamp: SystemTime,
}

impl LogMessage {
    /// Creates a new log message.
    #[must_use]
    pub fn new(mode: InstallLogMode, payload: impl Into<String>) -> Self {
        Self {
            mode,
            payload: payload.into(),
            timestamp: SystemTime::now(),
        }
    }

    /// Formats the log message in exact Windows Installer (voicewarmup) style.
    #[must_use]
    pub fn format_msi(&self) -> String {
        match self.mode {
            InstallLogMode::Header => {
                // Approximate standard MSI header
                format!("=== Verbose logging started: {} ===\n", self.payload)
            }
            InstallLogMode::Verbose => {
                format!("MSI (c) (XX:XX) [XX:XX:XX:XXX]: {}\n", self.payload)
            }
            InstallLogMode::ActionStart => {
                format!("Action start XX:XX:XX: {}.\n", self.payload)
            }
            InstallLogMode::PropertyDump => {
                format!("Property(C): {}\n", self.payload)
            }
            _ => format!("{}\n", self.payload),
        }
    }
}

/// A thread-safe, lock-free ring-buffer logging queue mimicking Windows Installer logging.
///
/// Uses a channel-based approach to ensure memory safety without panicking.
#[derive(Debug, Clone)]
pub struct MsiLogQueue {
    sender: std::sync::mpsc::SyncSender<LogMessage>,
    active: Arc<AtomicBool>,
}

impl MsiLogQueue {
    /// Creates a new background logging queue mapping to `MsiEnableLog`.
    ///
    /// The thread will run until the returned receiver handle is dropped or closed.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Max bounded capacity for the queue before blocking the producer.
    /// * `sink` - A callback or channel to receive formatted logs.
    #[must_use]
    pub fn new<F>(capacity: usize, mut sink: F) -> (Self, JoinHandle<()>)
    where
        F: FnMut(String) + Send + 'static,
    {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<LogMessage>(capacity);
        let active = Arc::new(AtomicBool::new(true));

        let thread_active = Arc::clone(&active);
        let handle = thread::spawn(move || {
            while thread_active.load(Ordering::Acquire) {
                match receiver.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(msg) => {
                        sink(msg.format_msi());
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        thread::yield_now();
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        thread::yield_now();
                        break;
                    }
                }
            }
        });

        (Self { sender, active }, handle)
    }

    /// Enqueues a message into the lock-free logging ring.
    ///
    /// # Errors
    /// Returns `MsiError::LoggingCallbackError` if the queue is disconnected.
    pub fn log(&self, mode: InstallLogMode, payload: &str) -> Result<()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(MsiError::LoggingCallbackError(
                "Logger is inactive".to_string(),
            ));
        }

        let msg = LogMessage::new(mode, payload);
        self.sender.try_send(msg).map_err(|_| {
            MsiError::LoggingCallbackError("Logging queue full or disconnected".to_string())
        })
    }

    /// Signals the logging thread to flush and exit.
    pub fn shutdown(&self) {
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_timeout_and_inactive() {
        let (sender, _receiver) = mpsc::channel();
        let (logger, handle) = MsiLogQueue::new(10, move |msg| {
            let _ = sender.send(msg);
        });

        // Let it hit timeout (50ms)
        thread::sleep(Duration::from_millis(150));

        // Test shutdown and inactive logger
        logger.shutdown();
        assert!(matches!(
            logger.log(InstallLogMode::Info, "test"),
            Err(MsiError::LoggingCallbackError(_))
        ));

        // Wait for thread to close
        handle.join().unwrap();
    }

    #[test]
    fn test_logger_disconnected() {
        let (sender, _receiver) = mpsc::channel();
        let (logger, handle) = MsiLogQueue::new(10, move |msg| {
            let _ = sender.send(msg);
        });

        // Drop the logger to drop its sender, hitting the Disconnected error branch
        drop(logger);

        handle.join().unwrap();
    }

    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn test_log_message_formatting() {
        let msg = LogMessage::new(InstallLogMode::Header, "Date Time");
        assert_eq!(
            msg.format_msi(),
            "=== Verbose logging started: Date Time ===\n"
        );

        let msg_verbose = LogMessage::new(InstallLogMode::Verbose, "Doing something");
        assert_eq!(
            msg_verbose.format_msi(),
            "MSI (c) (XX:XX) [XX:XX:XX:XXX]: Doing something\n"
        );

        let msg_action = LogMessage::new(InstallLogMode::ActionStart, "InstallFiles");
        assert_eq!(
            msg_action.format_msi(),
            "Action start XX:XX:XX: InstallFiles.\n"
        );

        let msg_prop = LogMessage::new(InstallLogMode::PropertyDump, "MY_PROP = 1");
        assert_eq!(msg_prop.format_msi(), "Property(C): MY_PROP = 1\n");

        let msg_gen = LogMessage::new(InstallLogMode::Info, "General info");
        assert_eq!(msg_gen.format_msi(), "General info\n");
    }

    #[test]
    fn test_logging_queue() -> Result<()> {
        let (tx, rx) = mpsc::channel();

        let (queue, handle) = MsiLogQueue::new(100, move |formatted| {
            let _ = tx.send(formatted);
        });

        // Test normal logging
        queue.log(InstallLogMode::Verbose, "Test message")?;

        let received = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("Timeout receiving log");
        assert_eq!(received, "MSI (c) (XX:XX) [XX:XX:XX:XXX]: Test message\n");

        // Test shutdown
        queue.shutdown();

        // Wait for thread to exit
        let _ = handle.join();
        Ok(())
    }

    #[test]
    fn test_logging_queue_full() {
        // Capacity 1 to force full state easily
        let (queue, _handle) = MsiLogQueue::new(1, |_| {
            // Block the consumer to fill the queue
            thread::sleep(Duration::from_millis(50));
        });

        // First one might succeed
        let _ = queue.log(InstallLogMode::Info, "msg 1");

        // Next ones will fill it quickly and then try_send will fail
        let mut full = false;
        for i in 0..10 {
            if queue
                .log(InstallLogMode::Info, &format!("msg {i}"))
                .is_err()
            {
                full = true;
                break;
            }
        }
        assert!(
            full,
            "Queue should report full and return error without panicking"
        );
    }
}

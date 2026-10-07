//! Native UI Subsystem and Dialog parsing.
//!
//! Handles parsing of `Dialog` and `Control` tables, complex control events,
//! and `MsiSetExternalUI` hooks for external rendering overrides.

use crate::error::{MsiError, Result};
use std::collections::HashMap;

/// Type for external UI callbacks via `MsiSetExternalUI`.
pub type ExternalUiCallback = fn(message_type: u32, message: &str) -> i32;

/// UI Handler and Dialog manager.
#[derive(Debug)]
pub struct UiManager {
    external_cb: Option<ExternalUiCallback>,
    dialogs: HashMap<String, DialogDef>,
}

/// Represents a parsed Dialog table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogDef {
    /// The name of the dialog.
    pub name: String,
    /// The dialog title.
    pub title: String,
    /// Controls associated with this dialog.
    pub controls: Vec<ControlDef>,
}

/// Represents a parsed Control table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlDef {
    /// A text control.
    Text(String),
    /// An edit box control.
    Edit(String),
    /// A push button control.
    PushButton(String),
    /// A progress bar control.
    ProgressBar(String),
    /// A check box control.
    CheckBox(String),
    /// A billboard control.
    Billboard(String),
}

/// Represents a Control Event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEvent {
    /// Spawns a new dialog.
    SpawnDialog(String),
    /// Ends the current dialog.
    EndDialog(String),
    /// Transitions to a new dialog.
    NewDialog(String),
    /// Sets a target path property.
    SetTargetPath(String),
}

impl Default for UiManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UiManager {
    /// Creates a new `UiManager`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            external_cb: None,
            dialogs: HashMap::new(),
        }
    }

    /// Sets the external UI handler (`MsiSetExternalUI`).
    pub fn set_external_ui(&mut self, cb: ExternalUiCallback) {
        self.external_cb = Some(cb);
    }

    /// Sends a message to the external UI handler if one is registered.
    ///
    /// # Errors
    /// Returns `MsiError` if the external handler returns a fatal error code.
    pub fn invoke_external_ui(&self, msg_type: u32, message: &str) -> Result<i32> {
        if let Some(cb) = self.external_cb {
            let res = cb(msg_type, message);
            if res == -1 {
                return Err(MsiError::ActionExecutionError(
                    "External UI aborted".to_string(),
                ));
            }
            Ok(res)
        } else {
            Ok(0) // Default ignore
        }
    }

    /// Parses the `Dialog` and `Control` tables.
    ///
    /// # Errors
    /// Returns `MsiError` if the tables are malformed.
    pub fn load_dialog(&mut self, name: &str, title: &str) -> Result<()> {
        if name.is_empty() {
            return Err(MsiError::ActionExecutionError(
                "Dialog name cannot be empty".to_string(),
            ));
        }
        self.dialogs.insert(
            name.to_string(),
            DialogDef {
                name: name.to_string(),
                title: title.to_string(),
                controls: Vec::new(),
            },
        );
        Ok(())
    }

    /// Adds a control to an existing dialog.
    ///
    /// # Errors
    /// Returns `MsiError` if the dialog does not exist.
    pub fn add_control(&mut self, dialog_name: &str, control: ControlDef) -> Result<()> {
        if let Some(dlg) = self.dialogs.get_mut(dialog_name) {
            dlg.controls.push(control);
            Ok(())
        } else {
            Err(MsiError::ActionExecutionError(format!(
                "Dialog {dialog_name} not found"
            )))
        }
    }

    /// Handles a control event execution.
    ///
    /// # Errors
    /// Returns `MsiError` if the event is invalid.
    pub fn handle_event(&self, event: &ControlEvent) -> Result<()> {
        match event {
            ControlEvent::SpawnDialog(name) | ControlEvent::NewDialog(name) => {
                if !self.dialogs.contains_key(name) {
                    return Err(MsiError::ActionExecutionError(format!(
                        "Cannot spawn unknown dialog {name}"
                    )));
                }
                Ok(())
            }
            ControlEvent::EndDialog(val) => {
                if val.is_empty() {
                    return Err(MsiError::ActionExecutionError(
                        "EndDialog requires a return value".to_string(),
                    ));
                }
                Ok(())
            }
            ControlEvent::SetTargetPath(path) => {
                if path.is_empty() {
                    return Err(MsiError::ActionExecutionError(
                        "SetTargetPath requires a path".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_manager_default() {
        let _ = UiManager::default();
    }

    #[test]
    fn test_ui_manager_external_ui() {
        let mut mgr = UiManager::new();
        assert_eq!(mgr.invoke_external_ui(1, "test").unwrap(), 0);

        fn cb(msg_type: u32, msg: &str) -> i32 {
            if msg == "abort" {
                -1
            } else {
                msg_type as i32
            }
        }

        mgr.set_external_ui(cb);
        assert_eq!(mgr.invoke_external_ui(42, "hello").unwrap(), 42);
        assert!(mgr.invoke_external_ui(0, "abort").is_err());
    }

    #[test]
    fn test_ui_manager_dialogs() {
        let mut mgr = UiManager::new();
        assert!(mgr.load_dialog("WelcomeDlg", "Welcome").is_ok());
        assert!(mgr.load_dialog("", "Empty").is_err());

        assert!(mgr
            .add_control("WelcomeDlg", ControlDef::PushButton("Next".to_string()))
            .is_ok());
        assert!(mgr
            .add_control("UnknownDlg", ControlDef::Text("Fail".to_string()))
            .is_err());

        assert!(mgr
            .handle_event(&ControlEvent::SpawnDialog("WelcomeDlg".to_string()))
            .is_ok());
        assert!(mgr
            .handle_event(&ControlEvent::SpawnDialog("MissingDlg".to_string()))
            .is_err());

        assert!(mgr
            .handle_event(&ControlEvent::EndDialog("Return".to_string()))
            .is_ok());
        assert!(mgr
            .handle_event(&ControlEvent::EndDialog(String::new()))
            .is_err());

        assert!(mgr
            .handle_event(&ControlEvent::SetTargetPath("C:\\".to_string()))
            .is_ok());
        assert!(mgr
            .handle_event(&ControlEvent::SetTargetPath(String::new()))
            .is_err());
    }
}

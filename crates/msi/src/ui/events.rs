//! MSI Control Events and Condition Dispatcher.
//!
//! Grounded directly in official Windows Installer SDK specifications:
//! - Standard control event triggers:
//!   - `EndDialog`: Terminate dialog loop with return code (`Return`, `Exit`, `Retry`, `Ignore`).
//!   - `NewDialog`: Navigate forward or backward to a target dialog.
//!   - `SpawnDialog`: Launch a modal child dialog.
//!   - `SpawnWaitDialog`: Launch a modal waiting spinner dialog.
//!   - `SetProperty`: Set or update an installer property value.
//!   - `Reset`: Revert dialog controls back to original property values.
//!   - `DoAction`: Execute an immediate action or custom action directly on click.
//! - Dynamic `ControlCondition` actions:
//!   - `Default`, `Enable`, `Disable`, `Hide`, `Show`.
//! - Subscribed `EventMapping` entries (e.g. `SetProgress`).

use crate::error::{Error, Result};
use crate::execution::properties::EvaluationContext;

/// Dialog loop termination return codes for `EndDialog` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogReturnCode {
    /// Return to caller with success / proceed.
    Return,
    /// Exit installer execution completely (user cancel or finish).
    Exit,
    /// Retry previous action.
    Retry,
    /// Ignore and proceed.
    Ignore,
}

impl DialogReturnCode {
    /// Parses a dialog return code from its MSI argument string.
    ///
    /// # Arguments
    ///
    /// * `s` - Return code string (e.g. "Return", "Exit", "Retry", "Ignore").
    ///
    /// # Returns
    ///
    /// Parsed [`DialogReturnCode`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if argument is unknown.
    pub fn from_argument(s: &str) -> Result<Self> {
        match s {
            "Return" => Ok(Self::Return),
            "Exit" => Ok(Self::Exit),
            "Retry" => Ok(Self::Retry),
            "Ignore" => Ok(Self::Ignore),
            other => Err(Error::InvalidArgument {
                argument: "EndDialog.Argument".to_string(),
                reason: format!("Unknown EndDialog return code '{other}'"),
            }),
        }
    }

    /// Returns the standard MSI argument string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Return => "Return",
            Self::Exit => "Exit",
            Self::Retry => "Retry",
            Self::Ignore => "Ignore",
        }
    }
}

/// Standard control event types parsed from the `ControlEvent` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEventType {
    /// Terminates current dialog loop.
    EndDialog(DialogReturnCode),
    /// Replaces active dialog with new dialog.
    NewDialog(String),
    /// Spawns modal child dialog on top of active dialog.
    SpawnDialog(String),
    /// Spawns waiting dialog during long-running operation.
    SpawnWaitDialog(String),
    /// Sets public or private property value.
    SetProperty {
        /// Target property name.
        property: String,
        /// Formatted value string.
        value: String,
    },
    /// Resets all controls to initial property values.
    Reset,
    /// Executes an immediate or custom action.
    DoAction(String),
}

impl ControlEventType {
    /// Parses a control event from event name and argument string.
    ///
    /// # Arguments
    ///
    /// * `event` - Event name (e.g. "`EndDialog`", "`NewDialog`", "`SetProperty`", "`DoAction`").
    /// * `argument` - Event parameter / argument string.
    ///
    /// # Returns
    ///
    /// Parsed [`ControlEventType`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if event is unrecognized.
    pub fn parse(event: &str, argument: &str) -> Result<Self> {
        match event {
            "EndDialog" => {
                let code = DialogReturnCode::from_argument(argument)?;
                Ok(Self::EndDialog(code))
            }
            "NewDialog" => Ok(Self::NewDialog(argument.to_string())),
            "SpawnDialog" => Ok(Self::SpawnDialog(argument.to_string())),
            "SpawnWaitDialog" => Ok(Self::SpawnWaitDialog(argument.to_string())),
            "SetProperty" => {
                let (prop, val) = if let Some((p, v)) = argument.split_once('=') {
                    (p.trim().to_string(), v.trim().to_string())
                } else {
                    (argument.trim().to_string(), String::new())
                };
                Ok(Self::SetProperty {
                    property: prop,
                    value: val,
                })
            }
            "Reset" => Ok(Self::Reset),
            "DoAction" => Ok(Self::DoAction(argument.to_string())),
            other => Err(Error::InvalidArgument {
                argument: "ControlEvent.Event".to_string(),
                reason: format!("Unrecognized ControlEvent type '{other}'"),
            }),
        }
    }
}

/// Single control event definition loaded from the `ControlEvent` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlEvent {
    /// Dialog containing the trigger control.
    dialog: String,
    /// Control triggering the event.
    control: String,
    /// Event operation.
    event_type: ControlEventType,
    /// Optional conditional expression.
    condition: Option<String>,
    /// Order index among multiple events on the same control.
    ordering: i16,
}

impl ControlEvent {
    /// Creates a new [`ControlEvent`].
    ///
    /// # Arguments
    ///
    /// * `dialog` - Dialog name.
    /// * `control` - Control name.
    /// * `event_type` - Event type.
    /// * `condition` - Optional condition.
    /// * `ordering` - Execution order index.
    ///
    /// # Returns
    ///
    /// A new [`ControlEvent`].
    #[must_use]
    pub fn new(
        dialog: impl Into<String>,
        control: impl Into<String>,
        event_type: ControlEventType,
        condition: Option<String>,
        ordering: i16,
    ) -> Self {
        Self {
            dialog: dialog.into(),
            control: control.into(),
            event_type,
            condition,
            ordering,
        }
    }

    /// Returns the dialog name.
    #[must_use]
    pub fn dialog(&self) -> &str {
        &self.dialog
    }

    /// Returns the control name.
    #[must_use]
    pub fn control(&self) -> &str {
        &self.control
    }

    /// Returns the event type.
    #[must_use]
    pub const fn event_type(&self) -> &ControlEventType {
        &self.event_type
    }

    /// Returns the condition expression if any.
    #[must_use]
    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }

    /// Returns the execution order.
    #[must_use]
    pub const fn ordering(&self) -> i16 {
        self.ordering
    }

    /// Evaluates whether this event's condition is satisfied.
    ///
    /// # Arguments
    ///
    /// * `context` - Active evaluation context.
    ///
    /// # Returns
    ///
    /// `true` if condition is satisfied or empty, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if condition syntax is invalid.
    pub fn is_satisfied(&self, context: &EvaluationContext) -> Result<bool> {
        self.condition.as_ref().map_or(Ok(true), |cond| {
            if cond.trim().is_empty() || cond == "1" {
                Ok(true)
            } else if cond == "0" {
                Ok(false)
            } else {
                context.evaluate_condition(cond)
            }
        })
    }
}

/// Actions applied to controls when a `ControlCondition` evaluates to true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlConditionAction {
    /// Sets this control as the default button on the dialog.
    Default,
    /// Enables the control for user interaction.
    Enable,
    /// Disables the control.
    Disable,
    /// Hides the control.
    Hide,
    /// Shows the control.
    Show,
}

impl ControlConditionAction {
    /// Parses a condition action string.
    ///
    /// # Arguments
    ///
    /// * `s` - Action name (e.g. "Default", "Enable", "Disable", "Hide", "Show").
    ///
    /// # Returns
    ///
    /// Parsed [`ControlConditionAction`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if action is unknown.
    pub fn from_action(s: &str) -> Result<Self> {
        match s {
            "Default" => Ok(Self::Default),
            "Enable" => Ok(Self::Enable),
            "Disable" => Ok(Self::Disable),
            "Hide" => Ok(Self::Hide),
            "Show" => Ok(Self::Show),
            other => Err(Error::InvalidArgument {
                argument: "ControlCondition.Action".to_string(),
                reason: format!("Unknown ControlCondition action '{other}'"),
            }),
        }
    }
}

/// Dynamic condition modifying control visibility or enabled state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCondition {
    /// Dialog containing control.
    pub dialog: String,
    /// Control being modified.
    pub control: String,
    /// Action to execute if condition is true.
    pub action: ControlConditionAction,
    /// Condition expression string.
    pub condition: String,
}

/// Event subscription mapping linking a control attribute to an engine notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMapping {
    /// Dialog containing control.
    pub dialog: String,
    /// Control subscribed to event.
    pub control: String,
    /// Event name being subscribed (e.g. `SetProgress`).
    pub event: String,
    /// Attribute modified on event notification.
    pub attribute: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests parsing [`DialogReturnCode`] and [`ControlEventType`].
    #[test]
    fn test_control_events_parsing() {
        let codes = [
            ("Return", DialogReturnCode::Return),
            ("Exit", DialogReturnCode::Exit),
            ("Retry", DialogReturnCode::Retry),
            ("Ignore", DialogReturnCode::Ignore),
        ];

        for (s, expected) in codes {
            assert_eq!(DialogReturnCode::from_argument(s), Ok(expected));
            assert_eq!(expected.as_str(), s);
        }
        assert!(DialogReturnCode::from_argument("BadCode").is_err());

        // Parse events
        assert_eq!(
            ControlEventType::parse("EndDialog", "Exit"),
            Ok(ControlEventType::EndDialog(DialogReturnCode::Exit))
        );
        assert!(ControlEventType::parse("EndDialog", "BadCode").is_err());

        assert_eq!(
            ControlEventType::parse("NewDialog", "NextDlg"),
            Ok(ControlEventType::NewDialog("NextDlg".to_string()))
        );

        assert_eq!(
            ControlEventType::parse("SpawnDialog", "CancelDlg"),
            Ok(ControlEventType::SpawnDialog("CancelDlg".to_string()))
        );

        assert_eq!(
            ControlEventType::parse("SpawnWaitDialog", "WaitDlg"),
            Ok(ControlEventType::SpawnWaitDialog("WaitDlg".to_string()))
        );

        assert_eq!(
            ControlEventType::parse("SetProperty", "MY_PROP=123"),
            Ok(ControlEventType::SetProperty {
                property: "MY_PROP".to_string(),
                value: "123".to_string(),
            })
        );

        assert_eq!(
            ControlEventType::parse("SetProperty", "EMPTY_PROP"),
            Ok(ControlEventType::SetProperty {
                property: "EMPTY_PROP".to_string(),
                value: String::new(),
            })
        );

        assert_eq!(
            ControlEventType::parse("Reset", ""),
            Ok(ControlEventType::Reset)
        );

        assert_eq!(
            ControlEventType::parse("DoAction", "ValidateAction"),
            Ok(ControlEventType::DoAction("ValidateAction".to_string()))
        );

        assert!(ControlEventType::parse("UnknownEvent", "").is_err());
    }

    /// Tests `ControlEvent` condition evaluation and ordering.
    #[test]
    fn test_control_event_evaluation() {
        let mut context = EvaluationContext::new();
        context.set_property("ACCEPT_EULA", "1");

        let ev1 = ControlEvent::new(
            "LicenseDlg",
            "NextBtn",
            ControlEventType::NewDialog("InstallDirDlg".to_string()),
            Some(r#"ACCEPT_EULA = "1""#.to_string()),
            1,
        );

        assert_eq!(ev1.dialog(), "LicenseDlg");
        assert_eq!(ev1.control(), "NextBtn");
        assert_eq!(
            ev1.event_type(),
            &ControlEventType::NewDialog("InstallDirDlg".to_string())
        );
        assert_eq!(ev1.condition(), Some(r#"ACCEPT_EULA = "1""#));
        assert_eq!(ev1.ordering(), 1);
        assert_eq!(ev1.is_satisfied(&context), Ok(true));

        // Unsatisfied condition
        context.set_property("ACCEPT_EULA", "0");
        assert_eq!(ev1.is_satisfied(&context), Ok(false));

        // Empty condition defaults to satisfied
        let ev_empty = ControlEvent::new(
            "LicenseDlg",
            "BackBtn",
            ControlEventType::NewDialog("WelcomeDlg".to_string()),
            None,
            1,
        );
        assert_eq!(ev_empty.condition(), None);
        assert_eq!(ev_empty.is_satisfied(&context), Ok(true));

        let ev_ws = ControlEvent::new("D", "C", ControlEventType::Reset, Some("  ".to_string()), 1);
        assert_eq!(ev_ws.is_satisfied(&context), Ok(true));

        let ev_one = ControlEvent::new("D", "C", ControlEventType::Reset, Some("1".to_string()), 1);
        assert_eq!(ev_one.is_satisfied(&context), Ok(true));

        let ev_zero =
            ControlEvent::new("D", "C", ControlEventType::Reset, Some("0".to_string()), 1);
        assert_eq!(ev_zero.is_satisfied(&context), Ok(false));
    }

    /// Tests `ControlConditionAction` parsing and variants.
    #[test]
    fn test_control_condition_action() {
        assert_eq!(
            ControlConditionAction::from_action("Default"),
            Ok(ControlConditionAction::Default)
        );
        assert_eq!(
            ControlConditionAction::from_action("Enable"),
            Ok(ControlConditionAction::Enable)
        );
        assert_eq!(
            ControlConditionAction::from_action("Disable"),
            Ok(ControlConditionAction::Disable)
        );
        assert_eq!(
            ControlConditionAction::from_action("Hide"),
            Ok(ControlConditionAction::Hide)
        );
        assert_eq!(
            ControlConditionAction::from_action("Show"),
            Ok(ControlConditionAction::Show)
        );
        assert!(ControlConditionAction::from_action("BadAction").is_err());
    }
}

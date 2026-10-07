use derive_more::Display;
use std::fmt::Display;

/// The status returned by an executed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    /// Action completed successfully.
    Success,
    /// Action failed.
    Failure,
    /// Action was skipped or ignored.
    Skipped,
    /// User cancelled the action.
    UserExit,
    /// Action indicated a fatal error.
    FatalExit,
}

impl Display for ActionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Failure => write!(f, "Failure"),
            Self::Skipped => write!(f, "Skipped"),
            Self::UserExit => write!(f, "UserExit"),
            Self::FatalExit => write!(f, "FatalExit"),
        }
    }
}

/// A strongly-typed sequence name (e.g. "`InstallUISequence`").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub struct SequenceName(pub String);

impl From<&str> for SequenceName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_status_display() {
        assert_eq!(ActionStatus::Success.to_string(), "Success");
        assert_eq!(ActionStatus::Failure.to_string(), "Failure");
        assert_eq!(ActionStatus::Skipped.to_string(), "Skipped");
        assert_eq!(ActionStatus::UserExit.to_string(), "UserExit");
        assert_eq!(ActionStatus::FatalExit.to_string(), "FatalExit");
    }

    #[test]
    fn test_sequence_name_from_str() {
        let name: SequenceName = "InstallUISequence".into();
        assert_eq!(name.0, "InstallUISequence");
        assert_eq!(name.to_string(), "InstallUISequence");
    }
}

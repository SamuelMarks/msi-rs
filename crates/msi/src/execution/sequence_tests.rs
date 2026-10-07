use super::*;
use crate::error::Result;

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
    let seq = SequenceName::from("InstallExecuteSequence");
    assert_eq!(seq.0, "InstallExecuteSequence");
    assert_eq!(seq.to_string(), "InstallExecuteSequence");
}

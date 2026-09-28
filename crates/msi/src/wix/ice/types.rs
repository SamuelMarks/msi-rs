//! Types, structures, and severity levels for Internal Consistency Evaluator (ICE) validation.
//!
//! Grounded directly in the Windows Installer SDK and `WiX` validation engine specifications:
//! - [`IceSeverity`]: Severity classification (`Error`, `Warning`).
//! - [`IceReport`]: Diagnostic report output from an ICE rule evaluation.

use std::fmt;

/// Severity classification of an Internal Consistency Evaluator diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IceSeverity {
    /// Validation warning: potential issue or best practice deviation.
    Warning,
    /// Validation error: fatal consistency or specification violation.
    Error,
}

impl fmt::Display for IceSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Diagnostic report resulting from an ICE validation rule evaluation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IceReport {
    /// ICE identifier (e.g. "ICE01", "ICE03", "ICE48").
    pub ice: String,
    /// Whether this report represents a fatal error or a warning.
    pub is_error: bool,
    /// Human-readable diagnostic description.
    pub message: String,
    /// Target database table where the violation was detected, if applicable.
    pub table: Option<String>,
    /// Specific column name associated with the failure, if applicable.
    pub column: Option<String>,
}

impl fmt::Display for IceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = if self.is_error { "error" } else { "warning" };
        if let Some(ref tbl) = self.table {
            if let Some(ref col) = self.column {
                write!(
                    f,
                    "{}: {} in table '{}' column '{}': {}",
                    self.ice, severity, tbl, col, self.message
                )
            } else {
                write!(
                    f,
                    "{}: {} in table '{}': {}",
                    self.ice, severity, tbl, self.message
                )
            }
        } else {
            write!(f, "{}: {} - {}", self.ice, severity, self.message)
        }
    }
}

impl IceReport {
    /// Creates a new error-level [`IceReport`].
    ///
    /// # Arguments
    ///
    /// * `ice` - Unique ICE rule identifier (e.g. "ICE16").
    /// * `message` - Diagnostic message describing the rule violation.
    ///
    /// # Returns
    ///
    /// A new error [`IceReport`].
    #[must_use]
    pub fn error(ice: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ice: ice.into(),
            is_error: true,
            message: message.into(),
            table: None,
            column: None,
        }
    }

    /// Creates a new warning-level [`IceReport`].
    ///
    /// # Arguments
    ///
    /// * `ice` - Unique ICE rule identifier (e.g. "ICE45").
    /// * `message` - Diagnostic message describing the potential issue.
    ///
    /// # Returns
    ///
    /// A new warning [`IceReport`].
    #[must_use]
    pub fn warning(ice: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ice: ice.into(),
            is_error: false,
            message: message.into(),
            table: None,
            column: None,
        }
    }

    /// Associates a database table name with this report.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    ///
    /// # Returns
    ///
    /// Updated [`IceReport`].
    #[must_use]
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Associates a column name with this report.
    ///
    /// # Arguments
    ///
    /// * `column` - Target column name.
    ///
    /// # Returns
    ///
    /// Updated [`IceReport`].
    #[must_use]
    pub fn with_column(mut self, column: impl Into<String>) -> Self {
        self.column = Some(column.into());
        self
    }

    /// Returns the severity level of this report.
    ///
    /// # Returns
    ///
    /// [`IceSeverity::Error`] if `is_error` is true, otherwise [`IceSeverity::Warning`].
    #[must_use]
    pub const fn severity(&self) -> IceSeverity {
        if self.is_error {
            IceSeverity::Error
        } else {
            IceSeverity::Warning
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests creation, severity querying, and display formatting of `IceSeverity`.
    #[test]
    fn test_ice_severity() {
        assert_eq!(format!("{}", IceSeverity::Warning), "warning");
        assert_eq!(format!("{}", IceSeverity::Error), "error");
        assert!(IceSeverity::Error > IceSeverity::Warning);
    }

    /// Tests creation and builder methods of `IceReport`.
    #[test]
    fn test_ice_report_builders_and_display() {
        let err = IceReport::error("ICE01", "Missing required table")
            .with_table("Component")
            .with_column("Component");
        assert!(err.is_error);
        assert_eq!(err.severity(), IceSeverity::Error);
        assert_eq!(
            format!("{err}"),
            "ICE01: error in table 'Component' column 'Component': Missing required table"
        );

        let warn = IceReport::warning("ICE45", "Illegal filename").with_table("File");
        assert!(!warn.is_error);
        assert_eq!(warn.severity(), IceSeverity::Warning);
        assert_eq!(
            format!("{warn}"),
            "ICE45: warning in table 'File': Illegal filename"
        );

        let bare = IceReport::error("ICE99", "Circular directory hierarchy");
        assert_eq!(
            format!("{bare}"),
            "ICE99: error - Circular directory hierarchy"
        );
    }

    /// Tests ordering and hashing of `IceReport`.
    #[test]
    fn test_ice_report_ordering() {
        let r1 = IceReport::warning("ICE01", "A");
        let r2 = IceReport::error("ICE02", "B");
        assert!(r1 < r2);
    }
}

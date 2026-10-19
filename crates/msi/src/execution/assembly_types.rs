//! Strongly-typed processor architecture and assembly identity types for WinSxS and GAC deployment.

use crate::error::{MsiError, Result};
use std::fmt;

/// Strong type for processor architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessorArchitecture {
    /// x86 (32-bit).
    X86,
    /// AMD64 (64-bit).
    Amd64,
    /// ARM (32-bit).
    Arm,
    /// ARM64 (64-bit).
    Arm64,
    /// IA64.
    Ia64,
    /// Microsoft Intermediate Language (MSIL, `AnyCPU`).
    Msil,
    /// Neutral architecture.
    Neutral,
}

impl ProcessorArchitecture {
    /// Parses an architecture string.
    ///
    /// # Errors
    /// Returns `MsiError::SxSError` if architecture string is invalid.
    ///
    /// # Arguments
    ///
    /// * `arch` - The architecture string to parse.
    ///
    /// # Returns
    ///
    /// The parsed `ProcessorArchitecture`.
    pub fn parse(arch: &str) -> Result<Self> {
        match arch.to_lowercase().as_str() {
            "x86" => Ok(Self::X86),
            "amd64" => Ok(Self::Amd64),
            "arm" => Ok(Self::Arm),
            "arm64" => Ok(Self::Arm64),
            "ia64" => Ok(Self::Ia64),
            "msil" => Ok(Self::Msil),
            "neutral" | "" => Ok(Self::Neutral),
            _ => Err(MsiError::SxSError(format!(
                "Invalid processor architecture: {arch}"
            ))),
        }
    }
}

impl fmt::Display for ProcessorArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::X86 => "x86",
            Self::Amd64 => "amd64",
            Self::Arm => "arm",
            Self::Arm64 => "arm64",
            Self::Ia64 => "ia64",
            Self::Msil => "msil",
            Self::Neutral => "neutral",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessorArchitecture;

    #[test]
    fn test_processor_architecture_parsing() {
        assert_eq!(
            ProcessorArchitecture::parse("x86"),
            Ok(ProcessorArchitecture::X86)
        );
        assert_eq!(
            ProcessorArchitecture::parse("amd64"),
            Ok(ProcessorArchitecture::Amd64)
        );
        assert_eq!(
            ProcessorArchitecture::parse("arm"),
            Ok(ProcessorArchitecture::Arm)
        );
        assert_eq!(
            ProcessorArchitecture::parse("arm64"),
            Ok(ProcessorArchitecture::Arm64)
        );
        assert_eq!(
            ProcessorArchitecture::parse("ia64"),
            Ok(ProcessorArchitecture::Ia64)
        );
        assert_eq!(
            ProcessorArchitecture::parse("msil"),
            Ok(ProcessorArchitecture::Msil)
        );
        assert_eq!(
            ProcessorArchitecture::parse("neutral"),
            Ok(ProcessorArchitecture::Neutral)
        );
        assert_eq!(
            ProcessorArchitecture::parse(""),
            Ok(ProcessorArchitecture::Neutral)
        );

        assert!(ProcessorArchitecture::parse("invalid").is_err());
    }

    #[test]
    fn test_processor_architecture_display() {
        assert_eq!(ProcessorArchitecture::X86.to_string(), "x86");
        assert_eq!(ProcessorArchitecture::Amd64.to_string(), "amd64");
        assert_eq!(ProcessorArchitecture::Arm.to_string(), "arm");
        assert_eq!(ProcessorArchitecture::Arm64.to_string(), "arm64");
        assert_eq!(ProcessorArchitecture::Ia64.to_string(), "ia64");
        assert_eq!(ProcessorArchitecture::Msil.to_string(), "msil");
        assert_eq!(ProcessorArchitecture::Neutral.to_string(), "neutral");
    }

    #[test]
    fn test_processor_architecture_derives() {
        use std::hash::{Hash, Hasher};
        let arch = ProcessorArchitecture::X86;
        let arch2 = arch;
        assert_eq!(arch, arch2);

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        arch.hash(&mut hasher);
        let hash1 = hasher.finish();

        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        arch2.hash(&mut hasher2);
        let hash2 = hasher2.finish();
        assert_eq!(hash1, hash2);

        assert_eq!(format!("{arch:?}"), "X86");
    }
}

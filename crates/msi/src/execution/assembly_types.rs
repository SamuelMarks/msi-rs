//! Strongly-typed processor architecture and assembly identity types for WinSxS and GAC deployment.

use crate::database::summary_info::Architecture;
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
            "x86" | "intel" | "i386" | "i686" => Ok(Self::X86),
            "amd64" | "x64" | "x86_64" => Ok(Self::Amd64),
            "arm" | "armnt" => Ok(Self::Arm),
            "arm64" | "aarch64" => Ok(Self::Arm64),
            "ia64" | "intel64" => Ok(Self::Ia64),
            "msil" => Ok(Self::Msil),
            "neutral" | "" => Ok(Self::Neutral),
            _ => Err(MsiError::SxSError(format!(
                "Invalid processor architecture: {arch}"
            ))),
        }
    }
}

impl From<Architecture> for ProcessorArchitecture {
    fn from(arch: Architecture) -> Self {
        match arch {
            Architecture::X86 => Self::X86,
            Architecture::X64 => Self::Amd64,
            Architecture::Arm => Self::Arm,
            Architecture::Arm64 => Self::Arm64,
            Architecture::Ia64 => Self::Ia64,
            Architecture::Neutral => Self::Neutral,
        }
    }
}

impl TryFrom<ProcessorArchitecture> for Architecture {
    type Error = MsiError;

    fn try_from(arch: ProcessorArchitecture) -> Result<Self> {
        match arch {
            ProcessorArchitecture::X86 => Ok(Self::X86),
            ProcessorArchitecture::Amd64 => Ok(Self::X64),
            ProcessorArchitecture::Arm => Ok(Self::Arm),
            ProcessorArchitecture::Arm64 => Ok(Self::Arm64),
            ProcessorArchitecture::Ia64 => Ok(Self::Ia64),
            ProcessorArchitecture::Msil | ProcessorArchitecture::Neutral => Ok(Self::Neutral),
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
    use crate::database::summary_info::Architecture;

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
    fn test_processor_architecture_conversions() {
        for (arch, proc_arch) in [
            (Architecture::X86, ProcessorArchitecture::X86),
            (Architecture::X64, ProcessorArchitecture::Amd64),
            (Architecture::Arm, ProcessorArchitecture::Arm),
            (Architecture::Arm64, ProcessorArchitecture::Arm64),
            (Architecture::Ia64, ProcessorArchitecture::Ia64),
            (Architecture::Neutral, ProcessorArchitecture::Neutral),
        ] {
            assert_eq!(ProcessorArchitecture::from(arch), proc_arch);
            assert_eq!(Architecture::try_from(proc_arch), Ok(arch));
        }
        assert_eq!(
            Architecture::try_from(ProcessorArchitecture::Msil),
            Ok(Architecture::Neutral)
        );
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

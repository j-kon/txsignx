use std::{error::Error, fmt};

/// Sanitized errors contain no PSBT metadata or caller-supplied strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    InconsistentWalletContext,
    InvalidFeeRatio,
    DuplicateRuleCode,
    InvalidRuleCode,
    FindingCodeMismatch,
    InconsistentInspection,
    UnevaluableFeeState,
}
impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InconsistentWalletContext => "wallet context does not match inspection facts",
            Self::InvalidFeeRatio => "maximum fee ratio must be between 0 and 10000 basis points",
            Self::DuplicateRuleCode => "policy registry contains a duplicate rule code",
            Self::InvalidRuleCode => "policy registry contains an invalid rule code",
            Self::FindingCodeMismatch => "finding code does not match its registered rule",
            Self::InconsistentInspection => "inspection facts are internally inconsistent; policy evaluation was not completed",
            Self::UnevaluableFeeState => "fee calculation failed (negative, overflow, or other error); policy evaluation was not completed",
        })
    }
}
impl Error for PolicyError {}

use std::{error::Error, fmt};
/// Deliberately contains no caller strings or dependency error sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletError {
    ResourceLimit,
    InvalidExternalDescriptor,
    InvalidInternalDescriptor,
    InvalidNetwork,
    InvalidWindow,
    DescriptorTooLarge,
    NonRangedDescriptor,
    MultipathDescriptor,
    AmbiguousScripts,
    DerivationFailed,
    InvalidExpectedChange,
    DuplicateExpectedChange,
    InconsistentInspection,
    ContextMismatch,
}
impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ResourceLimit => "descriptor derivation exceeds the wallet resource budget",
            Self::InvalidExternalDescriptor => {
                "external descriptor is invalid for the configured network or public derivation"
            }
            Self::InvalidInternalDescriptor => {
                "internal descriptor is invalid for the configured network or public derivation"
            }
            Self::InvalidNetwork => "configured network is invalid",
            Self::InvalidWindow => "derivation window must be between 1 and 10000",
            Self::DescriptorTooLarge => "descriptor exceeds the 64 KiB input limit",
            Self::NonRangedDescriptor => "wallet descriptors must be ranged",
            Self::MultipathDescriptor => {
                "use separate single-path external and internal descriptors"
            }
            Self::AmbiguousScripts => "wallet descriptors produce ambiguous script matches",
            Self::DerivationFailed => "public descriptor derivation failed",
            Self::InvalidExpectedChange => "expected-change output index is out of range",
            Self::DuplicateExpectedChange => "expected-change output indexes must be unique",
            Self::InconsistentInspection => {
                "inspection facts are inconsistent; wallet classification was not completed"
            }
            Self::ContextMismatch => "wallet context does not match inspection facts",
        })
    }
}
impl Error for WalletError {}

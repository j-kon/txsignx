use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletContextUnavailableReason {
    MissingPrevoutContext,
    InvalidPrevoutContext,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalletOwnership {
    External {
        derivation_index: u32,
    },
    Internal {
        derivation_index: u32,
    },
    NoMatchWithinWindow,
    Unavailable {
        reason: WalletContextUnavailableReason,
    },
}

use thiserror::Error;

/// Sanitized boundary errors. Upstream PSBT errors can contain private metadata;
/// deliberately retain neither their values nor their error sources.
#[derive(Debug, Error)]
pub enum PsbtError {
    #[error("PSBT input is empty")]
    EmptyInput,
    #[error("PSBT text exceeds the {limit}-byte application safety limit")]
    TextTooLarge { limit: usize },
    #[error("decoded PSBT exceeds the {limit}-byte application safety limit")]
    DecodedTooLarge { limit: usize },
    #[error("invalid standard base64 PSBT text")]
    InvalidBase64,
    #[error("invalid PSBT magic or separator")]
    InvalidMagic,
    #[error("invalid or truncated PSBT key-value data")]
    InvalidPsbt,
    #[error("unsupported PSBT version; only version 0 (BIP174) is supported")]
    UnsupportedVersion,
    #[error("trailing data after PSBT")]
    TrailingData,
    #[error("PSBT exceeds a structural application resource limit")]
    ResourceLimit,
    #[error("unsigned transaction analysis failed: {0}")]
    Transaction(#[from] crate::AnalysisError),
}

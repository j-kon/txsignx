use thiserror::Error;
/// Static errors deliberately discard external response, URL and authentication text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NodeError {
    #[error("RPC requires an exact loopback HTTP endpoint with an explicit port")]
    InvalidEndpoint,
    #[error("RPC cookie must be a readable bounded regular file with valid cookie authentication")]
    InvalidCookie,
    #[error("Bitcoin Core RPC failed or returned an invalid response")]
    Rpc,
    #[error("unsupported configured or node-reported network")]
    UnsupportedNetwork,
    #[error("Bitcoin Core reports initial download or an unready chain")]
    NodeNotReady,
    #[error("chain tip remained unstable across three complete attempts")]
    UnstableChainTip,
    #[error("Bitcoin Core returned inconsistent UTXO observations")]
    InconsistentObservation,
    #[error("node context does not match the inspected PSBT")]
    ContextMismatch,
    #[error("inspection facts are inconsistent or exceed the node input limit")]
    InvalidInspection,
    #[error("PSBT is not finalized or checked transaction extraction failed")]
    Extraction,
    #[error("Bitcoin Core did not allow the candidate transaction")]
    MempoolRejected,
}

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
#[derive(Clone, Copy, Debug)]
pub struct ApiError(pub StatusCode, pub &'static str, pub &'static str);
impl ApiError {
    pub const INVALID: Self = Self(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
        "Invalid input or context.",
    );
    pub const INVALID_TRANSACTION: Self = Self(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_transaction",
        "Invalid raw transaction.",
    );
    pub const INVALID_NETWORK: Self = Self(
        StatusCode::BAD_REQUEST,
        "invalid_network",
        "Invalid or unsupported Bitcoin network.",
    );
    pub const INVALID_TXID: Self = Self(
        StatusCode::BAD_REQUEST,
        "invalid_txid",
        "Invalid transaction ID format.",
    );
    pub const INVALID_BLOCK_HASH: Self = Self(
        StatusCode::BAD_REQUEST,
        "invalid_block_hash",
        "Invalid block hash: must be a 64-character hexadecimal string.",
    );
    pub const BLOCK_NOT_FOUND: Self = Self(
        StatusCode::NOT_FOUND,
        "block_not_found",
        "Block not found in blockchain.",
    );
    pub const TRANSACTION_NOT_FOUND: Self = Self(
        StatusCode::NOT_FOUND,
        "transaction_not_found",
        "Transaction not found in blockchain or mempool.",
    );
    pub const NODE_NOT_CONFIGURED: Self = Self(
        StatusCode::BAD_REQUEST,
        "node_not_configured",
        "Bitcoin Core node is not configured.",
    );
    pub const INVALID_CONTEXT: Self = Self(
        StatusCode::BAD_REQUEST,
        "invalid_context",
        "Invalid request context or mutually exclusive parameters.",
    );
    pub const JSON: Self = Self(
        StatusCode::BAD_REQUEST,
        "invalid_json",
        "Invalid JSON request.",
    );
    pub const SIZE: Self = Self(
        StatusCode::PAYLOAD_TOO_LARGE,
        "size_limit",
        "Request or response exceeds size limit.",
    );
    pub const MEDIA: Self = Self(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "Content-Type must be application/json.",
    );
    pub const MISSING: Self = Self(StatusCode::NOT_FOUND, "not_found", "Resource not found.");
    pub const METHOD: Self = Self(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "Method not allowed.",
    );
    pub const NODE: Self = Self(
        StatusCode::SERVICE_UNAVAILABLE,
        "node_unavailable",
        "Configured node is unavailable.",
    );
    pub const BUSY: Self = Self(StatusCode::SERVICE_UNAVAILABLE, "busy", "Service is busy.");
    pub const TIMEOUT: Self = Self(StatusCode::GATEWAY_TIMEOUT, "timeout", "Request timed out.");
    pub const INTERNAL: Self = Self(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "Internal service failure.",
    );
    pub const FORBIDDEN: Self = Self(
        StatusCode::FORBIDDEN,
        "forbidden",
        "Request origin or host is not allowed.",
    );
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(serde_json::json!({"error":{"code":self.1,"message":self.2}})),
        )
            .into_response()
    }
}

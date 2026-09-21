//! Local HTTP presentation boundary for the Rust security engine.
pub fn app() -> axum::Router {
    axum::Router::new().route("/api/v1/health", axum::routing::get(|| async {
        axum::Json(serde_json::json!({"status":"ok","service":"txsignx-api","version":env!("CARGO_PKG_VERSION")}))
    }))
}

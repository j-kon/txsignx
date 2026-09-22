use crate::{AppState, config::*, error::ApiError, request};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
pub(crate) fn text_limit(text: &str) -> Result<(), ApiError> {
    if text.len() > TEXT_BYTES {
        Err(ApiError::SIZE)
    } else {
        Ok(())
    }
}
struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > RESPONSE_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("output limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn json(value: &impl Serialize) -> Result<Response, ApiError> {
    let mut output = Bounded(Vec::new());
    serde_json::to_writer(&mut output, value).map_err(|_| ApiError::SIZE)?;
    Ok(([(header::CONTENT_TYPE, "application/json")], output.0).into_response())
}
fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(bytes).map_err(|_| ApiError::JSON)
}
pub(crate) async fn dispatch(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, ApiError> {
    let path = req.uri().path().to_owned();
    let method = req.method().clone();
    let analysis = matches!(
        path.as_str(),
        "/api/v1/transactions/inspect" | "/api/v1/psbt/inspect" | "/api/v1/psbt/preflight"
    );
    if analysis && req.uri().query().is_some() {
        return Err(ApiError::JSON);
    }
    if analysis {
        if method != axum::http::Method::POST {
            return Err(ApiError::METHOD);
        }
        let media = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !media
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/json")
        {
            return Err(ApiError::MEDIA);
        }
        let bytes = to_bytes(req.into_body(), BODY_BYTES)
            .await
            .map_err(|_| ApiError::SIZE)?;
        let permit = state
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError::BUSY)?;
        return tokio::task::spawn_blocking(move || {
            // Permit belongs to the blocking job, not the HTTP future that can time out.
            let _permit = permit;
            match path.as_str() {
                "/api/v1/transactions/inspect" => {
                    let r: request::Transaction = parse(&bytes)?;
                    text_limit(&r.raw_transaction)?;
                    json(
                        &txsignx_core::analyze_transaction(&r.raw_transaction)
                            .map_err(|_| ApiError::INVALID)?,
                    )
                }
                "/api/v1/psbt/inspect" => {
                    let r: request::Psbt = parse(&bytes)?;
                    text_limit(&r.psbt)?;
                    json(&txsignx_core::analyze_psbt(&r.psbt).map_err(|_| ApiError::INVALID)?)
                }
                _ => json(&crate::preflight::run(parse(&bytes)?, &state.config)?),
            }
        })
        .await
        .map_err(|_| ApiError::INTERNAL)?;
    }
    let known = matches!(
        path.as_str(),
        "/api/v1/health" | "/api/v1/capabilities" | "/api/v1/policies"
    ) || path.starts_with("/api/v1/policies/");
    if !known {
        return Err(ApiError::MISSING);
    }
    if method != axum::http::Method::GET {
        return Err(ApiError::METHOD);
    }
    match path.as_str() {
        "/api/v1/health" => json(
            &serde_json::json!({"status":"ok","service":"txsignx-api","version":env!("CARGO_PKG_VERSION")}),
        ),
        "/api/v1/capabilities" => json(
            &serde_json::json!({"raw_transaction_inspection":true,"psbt_inspection":true,"preflight":true,"psbt_v0_inspection":true,"policy_preflight":true,"psbt_v2":false,"wallet_context":true,"node_context_available":state.config.node.is_some(),"signing":false,"finalization":false,"broadcast_via_api":false,"active_rules":txsignx_policy::rule_catalog().map_err(|_| ApiError::INTERNAL)?.active_rules.len(),"deferred_rules":txsignx_policy::rule_catalog().map_err(|_| ApiError::INTERNAL)?.deferred_rules.len(),"limits":{"text_bytes":TEXT_BYTES,"body_bytes":BODY_BYTES,"response_bytes":RESPONSE_BYTES,"concurrent_requests":4,"workers":2,"timeout_seconds":state.config.request_timeout.as_secs_f64()}}),
        ),
        _ => {
            let catalog = txsignx_policy::rule_catalog().map_err(|_| ApiError::INTERNAL)?;
            if path == "/api/v1/policies" {
                return json(&catalog);
            }
            let code = path
                .strip_prefix("/api/v1/policies/")
                .ok_or(ApiError::MISSING)?;
            if let Some(rule) = catalog.active_rules.iter().find(|r| r.code == code) {
                return json(rule);
            }
            if let Some(rule) = catalog.deferred_rules.iter().find(|r| r.code == code) {
                return json(rule);
            }
            Err(ApiError::MISSING)
        }
    }
}

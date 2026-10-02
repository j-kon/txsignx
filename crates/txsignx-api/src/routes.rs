use crate::{AppState, config::*, error::ApiError, request};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{io::Write, sync::Arc};
pub(crate) fn text_limit(text: &str) -> Result<(), ApiError> {
    if text.len() > TEXT_BYTES {
        Err(ApiError::SIZE)
    } else {
        Ok(())
    }
}
pub(crate) async fn ws_stream(
    State(state): State<AppState>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let live = state.live.as_ref().ok_or(ApiError::NODE_NOT_CONFIGURED)?;
    Ok(Arc::clone(live).handle_ws_upgrade(ws).await)
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
        if path == "/api/v1/transactions/inspect" {
            let r: request::Transaction = parse(&bytes)?;
            match (r.raw_transaction, r.txid) {
                (Some(raw_hex), None) => {
                    text_limit(&raw_hex)?;
                    let parsed_network = match r.network.as_deref() {
                        Some(net_str) => {
                            let conf: txsignx_wallet::ConfiguredNetwork =
                                net_str.parse().map_err(|_| ApiError::INVALID_NETWORK)?;
                            Some(conf.bitcoin_network())
                        }
                        None => None,
                    };
                    return tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        let tx = txsignx_core::decode_transaction(&raw_hex)
                            .map_err(|_| ApiError::INVALID_TRANSACTION)?;
                        let context = txsignx_core::transaction::TransactionAnalysisContext {
                            network: parsed_network,
                            chain_context: None,
                            resolved_prevouts: None,
                        };
                        let report =
                            txsignx_core::transaction::analyze_decoded_transaction_with_context(
                                &tx,
                                Some(&context),
                            )
                            .map_err(|_| ApiError::INVALID_TRANSACTION)?;
                        json(&report)
                    })
                    .await
                    .map_err(|_| ApiError::INTERNAL)?;
                }
                (None, Some(txid_str)) => {
                    text_limit(&txid_str)?;
                    if r.network.is_some() {
                        return Err(ApiError::INVALID_CONTEXT);
                    }
                    let txid: bitcoin::Txid =
                        txid_str.parse().map_err(|_| ApiError::INVALID_TXID)?;

                    let (node_tx, prevouts, network_name, btc_net) =
                        if let Some(live) = state.live.as_ref() {
                            let (tx, prevouts, _) = live.inspect_transaction(&txid).await?;
                            let net_name = live.network_name();
                            let btc_net = match net_name.as_str() {
                                "bitcoin" | "mainnet" => bitcoin::Network::Bitcoin,
                                "testnet" => bitcoin::Network::Testnet,
                                "testnet4" => bitcoin::Network::Testnet4,
                                "signet" => bitcoin::Network::Signet,
                                _ => bitcoin::Network::Regtest,
                            };
                            (tx, prevouts, net_name, btc_net)
                        } else if let Some(node) = state.config.node.as_ref() {
                            let (tx, prevouts, _) = node
                                .inspect_transaction(&txid)
                                .map_err(|_| ApiError::NODE)?;
                            (
                                tx,
                                prevouts,
                                node.network.name().to_string(),
                                node.network.bitcoin_network(),
                            )
                        } else {
                            return Err(ApiError::NODE_NOT_CONFIGURED);
                        };

                    return tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        let status = if node_tx.confirmations.unwrap_or(0) > 0 {
                            txsignx_core::transaction::TransactionConfirmationStatus::Confirmed
                        } else {
                            txsignx_core::transaction::TransactionConfirmationStatus::Mempool
                        };
                        let chain_context = txsignx_core::transaction::TransactionChainContext {
                            network: network_name,
                            status,
                            confirmations: node_tx.confirmations,
                            block_hash: node_tx.block_hash.map(|h| h.to_string()),
                        };
                        let context = txsignx_core::transaction::TransactionAnalysisContext {
                            network: Some(btc_net),
                            chain_context: Some(chain_context),
                            resolved_prevouts: Some(prevouts),
                        };
                        let report =
                            txsignx_core::transaction::analyze_decoded_transaction_with_context(
                                &node_tx.transaction,
                                Some(&context),
                            )
                            .map_err(|_| ApiError::INVALID_TRANSACTION)?;
                        json(&report)
                    })
                    .await
                    .map_err(|_| ApiError::INTERNAL)?;
                }
                _ => return Err(ApiError::INVALID_CONTEXT),
            }
        }

        return tokio::task::spawn_blocking(move || {
            // Permit belongs to the blocking job, not the HTTP future that can time out.
            let _permit = permit;
            match path.as_str() {
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
        "/api/v1/health"
            | "/api/v1/capabilities"
            | "/api/v1/policies"
            | "/api/v1/live/snapshot"
            | "/api/v1/blocks/recent"
            | "/api/v1/mempool/summary"
    ) || path.starts_with("/api/v1/policies/")
        || path.starts_with("/api/v1/blocks/");
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
        "/api/v1/capabilities" => {
            let (live_source, live_source_label, network) = match &state.live {
                Some(live) => (
                    Some(live.source_name()),
                    Some(live.source_label()),
                    Some(live.network_name()),
                ),
                None => (None, None, None),
            };
            json(&serde_json::json!({
                "raw_transaction_inspection":true,
                "transaction_explorer":true,
                "txid_inspection":true,
                "transaction_address_rendering":true,
                "psbt_inspection":true,
                "preflight":true,
                "psbt_v0_inspection":true,
                "policy_preflight":true,
                "psbt_v2":false,
                "wallet_context":true,
                "node_context_available":state.config.node.is_some(),
                "live_chain":state.live.is_some(),
                "live_stream":state.live.is_some(),
                "block_details":state.live.is_some(),
                "live_source":live_source,
                "live_source_label":live_source_label,
                "network":network,
                "signing":false,
                "finalization":false,
                "broadcast_via_api":false,
                "active_rules":txsignx_policy::rule_catalog().map_err(|_| ApiError::INTERNAL)?.active_rules.len(),
                "deferred_rules":txsignx_policy::rule_catalog().map_err(|_| ApiError::INTERNAL)?.deferred_rules.len(),
                "limits":{
                    "text_bytes":TEXT_BYTES,
                    "body_bytes":BODY_BYTES,
                    "response_bytes":RESPONSE_BYTES,
                    "concurrent_requests":4,
                    "workers":2,
                    "timeout_seconds":state.config.request_timeout.as_secs_f64()
                }
            }))
        }
        "/api/v1/live/snapshot" => {
            let live = state.live.as_ref().ok_or(ApiError::NODE_NOT_CONFIGURED)?;
            json(&live.get_snapshot().await?)
        }
        "/api/v1/blocks/recent" => {
            let live = state.live.as_ref().ok_or(ApiError::NODE_NOT_CONFIGURED)?;
            json(&live.get_recent_blocks().await?)
        }
        "/api/v1/mempool/summary" => {
            let live = state.live.as_ref().ok_or(ApiError::NODE_NOT_CONFIGURED)?;
            json(&live.get_mempool_summary().await?)
        }
        _ if path.starts_with("/api/v1/blocks/height/") => {
            let live = state.live.as_ref().ok_or(ApiError::NODE_NOT_CONFIGURED)?;
            let height_str = path.strip_prefix("/api/v1/blocks/height/").unwrap_or("");
            let height: u64 = height_str.parse().map_err(|_| ApiError::INVALID)?;
            let (offset, limit) = parse_block_query(req.uri().query());
            let details = live
                .get_block_details_by_height(height, offset, limit)
                .await?;
            json(&details)
        }
        _ if path.starts_with("/api/v1/blocks/") => {
            let live = state.live.as_ref().ok_or(ApiError::NODE_NOT_CONFIGURED)?;
            let hash_str = path.strip_prefix("/api/v1/blocks/").unwrap_or("");
            if hash_str.len() != 64 || !hash_str.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ApiError::INVALID_BLOCK_HASH);
            }
            let block_hash: bitcoin::BlockHash =
                hash_str.parse().map_err(|_| ApiError::INVALID_BLOCK_HASH)?;
            let (offset, limit) = parse_block_query(req.uri().query());
            let details = live.get_block_details(&block_hash, offset, limit).await?;
            json(&details)
        }
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

fn parse_block_query(query_str: Option<&str>) -> (usize, usize) {
    let mut offset = 0;
    let mut limit = 50;
    if let Some(q) = query_str {
        for part in q.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                match k {
                    "offset" => {
                        if let Ok(val) = v.parse::<usize>() {
                            offset = val;
                        }
                    }
                    "limit" => {
                        if let Ok(val) = v.parse::<usize>() {
                            limit = val.clamp(1, 100);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    (offset, limit)
}

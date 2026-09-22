use crate::{config::Config, error::ApiError, request::Preflight};
use txsignx_policy::{PolicyConfig, PolicyEngine, PreflightReport};
pub(crate) fn run(request: Preflight, config: &Config) -> Result<PreflightReport, ApiError> {
    super::routes::text_limit(&request.psbt)?;
    let options = request.policy.unwrap_or_default();
    let defaults = PolicyConfig::default();
    let policy_config = PolicyConfig {
        max_absolute_fee_sats: options
            .max_absolute_fee_sats
            .unwrap_or(defaults.max_absolute_fee_sats),
        max_fee_ratio_bps: options
            .max_fee_ratio_bps
            .unwrap_or(defaults.max_fee_ratio_bps),
    };
    policy_config.validate().map_err(|_| ApiError::INVALID)?;
    let use_node = request.node.is_some_and(|n| n.use_configured_node);
    let wallet = request
        .wallet
        .map(|w| {
            let network = w.network.parse().map_err(|_| ApiError::INVALID)?;
            let wallet = txsignx_wallet::WalletConfig::new(
                &w.external_descriptor,
                &w.internal_descriptor,
                network,
                w.derivation_window
                    .unwrap_or(txsignx_wallet::DEFAULT_DERIVATION_WINDOW),
            )
            .map_err(|_| ApiError::INVALID)?;
            Ok::<_, ApiError>((wallet, network, w.expected_change_outputs))
        })
        .transpose()?;
    if use_node {
        let (_, network, _) = wallet.as_ref().ok_or(ApiError::INVALID)?;
        let node = config.node.as_ref().ok_or(ApiError::NODE)?;
        if *network != node.network {
            return Err(ApiError::INVALID);
        }
    }
    let inspection = txsignx_core::analyze_psbt(&request.psbt).map_err(|_| ApiError::INVALID)?;
    let wallet_context = wallet
        .map(|(w, _, expected)| {
            txsignx_wallet::WalletIndex::new(w)
                .and_then(|i| i.classify(&inspection, &expected))
                .map_err(|_| ApiError::INVALID)
        })
        .transpose()?;
    let node_context = if use_node {
        Some(
            (config.node.as_ref().ok_or(ApiError::NODE)?.observe)(&inspection)
                .map_err(|_| ApiError::NODE)?,
        )
    } else {
        None
    };
    let policy = PolicyEngine::development()
        .map_err(|_| ApiError::INTERNAL)?
        .evaluate_with_context(
            &inspection,
            &policy_config,
            wallet_context.as_ref(),
            node_context.as_ref(),
        )
        .map_err(|_| ApiError::INVALID)?;
    Ok(PreflightReport {
        inspection,
        wallet_context,
        node_context,
        policy,
    })
}

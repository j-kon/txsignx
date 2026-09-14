use crate::psbt_input::{self, PsbtSource};
use clap::Args;
use std::{
    error::Error,
    io::{self, BufWriter, Write},
    process::ExitCode,
};
use txsignx_policy::{PolicyDecision::*, *};

#[derive(Args)]
pub struct PreflightArgs {
    #[command(flatten)]
    source: PsbtSource,
    #[command(flatten)]
    wallet: crate::wallet_input::WalletArgs,
    /// Emit inspection and policy JSON before returning the decision exit code.
    #[arg(long)]
    json: bool,
    /// Development policy maximum absolute fee in satoshis (equality allowed).
    #[arg(long,default_value_t=PolicyConfig::default().max_absolute_fee_sats,value_name="SATS")]
    max_absolute_fee_sats: u64,
    /// Development fee share limit: basis points of total input value, 0..10000.
    #[arg(long,default_value_t=PolicyConfig::default().max_fee_ratio_bps,value_name="BPS")]
    max_fee_ratio_bps: u16,
}

pub fn run(args: PreflightArgs) -> Result<ExitCode, Box<dyn Error>> {
    let config = PolicyConfig {
        max_absolute_fee_sats: args.max_absolute_fee_sats,
        max_fee_ratio_bps: args.max_fee_ratio_bps,
    };
    // Reject invalid configuration before reading files or blocking on stdin.
    config.validate()?;
    let wallet_config = args.wallet.config()?;
    let text = psbt_input::read(args.source)?;
    let inspection = txsignx_core::analyze_psbt(&text)?;
    let wallet_context = wallet_config
        .map(|config| {
            txsignx_wallet::WalletIndex::new(config)?
                .classify(&inspection, &args.wallet.expected_change_output)
        })
        .transpose()?;
    let policy = PolicyEngine::development()?.evaluate_with_wallet(
        &inspection,
        &config,
        wallet_context.as_ref(),
    )?;
    let decision = policy.decision;
    let report = PreflightReport {
        inspection,
        wallet_context,
        policy,
    };
    let mut stdout = BufWriter::new(io::stdout().lock());
    if args.json {
        serde_json::to_writer_pretty(&mut stdout, &report)?;
        writeln!(stdout)?;
    } else {
        write_report(&mut stdout, &report)?;
    }
    stdout.flush()?;
    Ok(ExitCode::from(match decision {
        Pass => 0,
        Review => 2,
        Block => 3,
    }))
}

fn decision_name(decision: PolicyDecision) -> &'static str {
    match decision {
        Pass => "PASS",
        Review => "REVIEW",
        Block => "BLOCK",
    }
}
fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "INFO",
        Severity::Low => "LOW",
        Severity::Medium => "MEDIUM",
        Severity::High => "HIGH",
        Severity::Critical => "CRITICAL",
    }
}
fn risk_name(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "LOW",
        RiskLevel::Medium => "MEDIUM",
        RiskLevel::High => "HIGH",
        RiskLevel::Critical => "CRITICAL",
    }
}
fn write_report(out: &mut impl Write, report: &PreflightReport) -> io::Result<()> {
    writeln!(
        out,
        "TxSignX PSBT Preflight\n------------------------------------"
    )?;
    writeln!(
        out,
        "Unsigned TXID: {}\nInputs: {}\nOutputs: {}",
        report.inspection.unsigned_txid,
        report.inspection.input_count,
        report.inspection.output_count
    )?;
    match report.inspection.fee.fee_sats {
        Some(fee) => writeln!(out, "Fee: {fee} sats (from supplied UTXO context)")?,
        None => writeln!(out, "Fee: unavailable; see UTXO context findings")?,
    }
    writeln!(
        out,
        "\nPolicy decision: {}\nRisk level: {}\nFindings: {}",
        decision_name(report.policy.decision),
        risk_name(report.policy.risk_level),
        report.policy.finding_count
    )?;
    if let Some(wallet) = &report.wallet_context {
        crate::wallet_display::write_report(out, wallet)?;
    }
    writeln!(out, "\nRule evaluations")?;
    for evaluation in &report.policy.rule_evaluations {
        writeln!(
            out,
            "  {}: {:?}{}",
            evaluation.code,
            evaluation.status,
            evaluation
                .reason
                .map(|r| format!(" ({r:?})"))
                .unwrap_or_default()
        )?;
    }
    let config = &report.policy.config;
    writeln!(
        out,
        "\nActive development policy (not consensus limits or universal recommendations):\n  Maximum fee: {} sats\n  Maximum fee ratio: {}.{:02}% of total input value ({} bps)",
        config.max_absolute_fee_sats,
        config.max_fee_ratio_bps / 100,
        config.max_fee_ratio_bps % 100,
        config.max_fee_ratio_bps
    )?;
    if report.policy.decision == Pass {
        writeln!(
            out,
            "\nNo currently evaluated active policy requires review or blocking."
        )?;
    }
    for finding in &report.policy.findings {
        writeln!(
            out,
            "\n[{}] {} — {}",
            severity_name(finding.severity),
            finding.code,
            finding.title
        )?;
        match finding.location {
            FindingLocation::Global => writeln!(out, "Location: global")?,
            FindingLocation::Input { index } => writeln!(out, "Location: input {index}")?,
            FindingLocation::Output { index } => writeln!(out, "Location: output {index}")?,
        }
        writeln!(out, "{}", finding.message)?;
        if let Some(recommendation) = &finding.recommendation {
            writeln!(out, "Recommendation: {recommendation}")?;
        }
    }
    writeln!(out, "\nScope note: {}", report.policy.scope_note)
}

pub fn list(json: bool) -> Result<ExitCode, Box<dyn Error>> {
    let catalog = rule_catalog()?;
    let mut stdout = BufWriter::new(io::stdout().lock());
    if json {
        serde_json::to_writer_pretty(&mut stdout, &catalog)?;
        writeln!(stdout)?;
    } else {
        writeln!(
            stdout,
            "TxSignX Development Policy Rules\n------------------------------------"
        )?;
        for rule in catalog.active_rules {
            writeln!(
                stdout,
                "{}  {}  {}\n  {}\n  Required context: {}",
                rule.code,
                severity_name(rule.default_severity),
                rule.title,
                rule.description,
                rule.required_context.join(", ")
            )?;
        }
        writeln!(stdout, "\nRESERVED / DEFERRED — not evaluated")?;
        for rule in catalog.deferred_rules {
            writeln!(
                stdout,
                "{}  {}\n  Requires: {}",
                rule.code,
                rule.title,
                rule.required_context.join(", ")
            )?;
        }
        writeln!(stdout, "\n{POLICY_SCOPE}")?;
    }
    stdout.flush()?;
    Ok(ExitCode::SUCCESS)
}

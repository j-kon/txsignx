# Milestone 2 — PSBT inspection plan

Base: `ca65a88c9b2e38995aca086648b9cfe4ecdfbfc4`.
Branch: `milestone-2-psbt-inspection`. The supplied Milestone 2 brief is the
approved scope. Only this Rust repository may change; no PR, merge, history
rewrite, sibling changes, signing, finalizing, extracting, broadcasting, or
Milestone 3 policy implementation is authorized.

## Architecture and decisions

- Share `analyze_decoded_transaction(&Transaction)` with raw analysis, preserving
  all Milestone 1 report fields and safety checks.
- Use rust-bitcoin 0.32.102 PSBT v0/BIP174 parsing. Enable its existing `base64`
  feature and re-exported codec; no new direct runtime dependency or version
  upgrade. This enables the optional transitive base64 crate.
- `analyze_psbt(&str)` returns stable serializable facts. PSBT errors are typed
  and sanitized: upstream errors may contain proprietary keys or preimages and
  must not be exposed in Display, Debug, sources, or CLI diagnostics.
- Standard padded base64 only; trim surrounding ASCII whitespace, reject interior
  whitespace, raw binary, and implicit hex. Limit decoded data to 16 MiB and
  textual input to its base64 ceiling plus two bytes for a terminal CRLF.
- Before semantic parsing, scan framing without allocating values, using
  rust-bitcoin CompactSize decoding. Bound map/pair counts and the global
  unsigned transaction length; rust-bitcoin remains the semantic PSBT parser.
  Ensure the reader is exhausted after parsing; preserve unknown/proprietary
  entries internally and report counts only.
- Report UTXO source and consistency. Check non-witness TXID, vout, and value/
  script agreement when both forms occur. Resolve nothing from inconsistent
  metadata. Witness-only data is supplied context, not authenticated chain data.
- Only after consistency checks use checked fee arithmetic/rust-bitcoin's fee
  facility. Report available/missing/invalid/negative/overflow states with an
  optional integer fee. Never report final feerate, network, address, ownership,
  signature validity, or broadcast readiness.
- Signing states are structural: final markers win, then ECDSA/Taproot signature
  presence, then unsigned. Overall mixtures containing finalized and non-final
  inputs are mixed; unsigned plus partial inputs are partially signed.
- CLI sources are mutually exclusive positional base64, `--file`, or `--stdin`.
  File/stdin reads stop after the text ceiling plus one byte (error, no silent
  truncation). Formatting stays in the CLI. JSON stdout contains only reports.

## Implementation sequence

- [ ] Refactor decoded transaction analysis; prove raw JSON regression equivalence.
- [ ] Add bounded base64/PSBT parsing and sanitized errors with adversarial tests.
- [ ] Add report models and structural signing-state tests.
- [ ] Add UTXO consistency and absolute-fee inspection with deterministic tests.
- [ ] Assemble input/output/global reports with metadata counts and reused facts.
- [ ] Add CLI positional inspection and human output with real process tests.
- [ ] Add bounded file/stdin sources, JSON output and source-conflict tests.
- [ ] Add documented synthetic unsigned/partial fixtures and adversarial coverage.
- [ ] Review security/privacy, update README, run all checks and cargo audit.
- [ ] Record verified results, inspect scope/secrets and normally push the branch.

Use meaningful incremental conventional commits. Observe failing behavior tests
before implementing it; retain all 54 Milestone 1 tests. Independent review must
check UTXO/fee trust, parser allocation limits, privacy, CLI and documentation.
Only mark Milestone 2 complete after verification succeeds.

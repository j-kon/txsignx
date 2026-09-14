# Milestone 3 — deterministic policy engine implementation plan

**Goal:** Interpret existing PSBT inspection facts using explicit, reproducible
rules and configuration; expose scoped PASS / REVIEW / BLOCK results.
**Architecture:** core derives facts; policy reads `&PsbtReport`; CLI handles
bounded input and presentation. No policy dependency is added to core.
**Tech stack:** existing Rust workspace, serde, txsignx-core; no new crates.io
runtime dependencies and no dependency upgrades.
**Spec:** User-supplied “TxSignX Milestone 3 — Deterministic Security Policy
Engine” brief, approved 2026-09-14. This plan records its implementation choices.

## Constraints and decisions

- Base `4fcde9df8c523d58dc19700534dea43c78e4a776`; branch
  `milestone-3-policy-engine`. Only the Rust repository changes. Normal commits
  and push; no PR, merge, history rewriting, sibling changes or Milestone 4.
- All rules consume shared, immutable inspection facts. No parsing, network I/O,
  randomness, time dependence, floating-point comparisons or AI in policy logic.
- `PolicyConfig` defaults: 100,000 maximum absolute fee sats, 1,000 maximum fee
  ratio basis points. Validate ratio <=10,000; zero is allowed. These are
  development policy defaults, not consensus or universal recommendations.
- Fee ratio is fee / (output total + fee). Widen both addition and products to
  u128. Equality never triggers a fee threshold finding.
- Active registration order: TG002, TG003, TG009, TG010, TG011, TG012, TG013,
  TG014. Within each rule sort locations global, input index, output index.
- Trait-based rule registry rejects duplicate codes using a BTreeSet. Store
  metadata once; active rules only appear in `evaluated_rules`. CLI listings
  use the same metadata. Deferred codes have inactive metadata, no evaluator.
- Severity drives decision and risk by maximum, never accumulated score.
  Critical => BLOCK; High/Medium => REVIEW; Info/Low/none => PASS.
- Numeric explicit sighash 0/1 is ordinary; every other explicitly supplied
  value requires review, including unrecognized numeric values. No cryptographic
  validity or script-version compatibility is inferred.
- Unknown/proprietary metadata yields one aggregated count-only finding. Use
  u128 counts to avoid overflow even for hand-constructed report boundaries.
- TG013 inspects outputs and valid resolved prevouts only. TG009 reports invalid
  supplied UTXO context per input; TG010 reports missing context per input.
- Missing/invalid UTXO fee states skip fee rules and remain visible via TG010/
  TG009. Negative/overflow/other fee calculation failures return a typed policy
  evaluation error (no PASS), because no active rule describes those states.
  Reject inconsistent fee-state/value/context combinations at the engine boundary.
- Policy JSON records configuration and a scope note alongside findings. It never
  means “safe to sign”. Preflight JSON wraps `inspection` and `policy` without
  including raw PSBT, signatures or origins.
- Preflight exit codes: PASS 0, REVIEW 2, BLOCK 3; input/config/runtime errors 1.
  Reports flush before decision exit. Existing inspect success/JSON unchanged;
  sanitized argument errors use 1 consistently to avoid ambiguity with REVIEW.

## Tasks and verification

For each implementation task, add behavior tests, observe the intended failure,
implement, rerun the focused suite, and commit the coherent result.

- [ ] Models and configuration: create `crates/txsignx-policy/Cargo.toml`,
  `src/{lib,model,config,error}.rs`; register crate in root Cargo.toml.
  `PolicyConfig::validate() -> Result<(), PolicyError>`; serde snake_case enums
  and tagged locations. Tests: defaults, bps 0/10,000/10,001 and JSON shape.
- [ ] Engine: `src/engine.rs`; `PolicyRule::{metadata,evaluate}`,
  `PolicyContext<'a>`, `PolicyEngine::new(Vec<Box<dyn PolicyRule>>)`,
  `evaluate(&PsbtReport, &PolicyConfig) -> Result<PolicyReport, PolicyError>`.
  Test each severity mapping, empty results, dominance, duplicate rejection,
  inactive rules, stable ordering, immutability and fee-state errors.
- [ ] Fee rules: `src/rules/fees.rs`; TG002 `fee > threshold`, TG003
  `u128(fee)*10000 > (u128(outputs)+u128(fee))*u128(bps)`.
  Test below/equal/above, 800k/900k, zero denominator, u64 boundaries,
  unavailable fees and threshold overrides.
- [ ] Context rules: `src/rules/utxo.rs`; input-scoped invalid/missing findings.
  Test each invalid status, missing, valid, mixed invalid/missing inputs, fee
  suppression and exact input locations.
- [ ] Sighash rule: `src/rules/sighash.rs`; test absent, 0, 1, 2, 3, 0x81,
  0x82, 0x83 and nonstandard numeric values, independently of display strings.
- [ ] Metadata/script rules: `src/rules/metadata.rs`, `scripts.rs`; aggregate
  extension counts; unknown templates per input/output; positive OP_RETURN
  output value blocks. Test all metadata locations, large counts, unknown vs
  recognized templates, zero/nonzero OP_RETURN, deterministic rule ordering.
- [ ] Registry and fixtures: `src/rules/mod.rs`, `src/registry.rs`; metadata for
  eight active and six reserved rules. Programmatic public dummy fixture
  generation in a core example and checked-in `fixtures/policy/*.b64` for PASS,
  absolute fee, percentage fee, 800k fee demo, missing/invalid context, sighash,
  OP_RETURN and unknown scripts; document origins and expected codes.
- [ ] CLI: extend existing main and reuse `psbt_input::PsbtSource`; add
  `preflight.rs` presentation, policy list, config flags. Integration tests for
  all sources, JSON, overrides, invalid bps, exit codes, errors, privacy and
  unchanged inspect behavior. No duplicated bounded-reader implementation.
- [ ] Security review: independent source review plus local arithmetic,
  determinism, privacy, mutation and unavailable-fee checks. Fix real findings
  with regression tests. Scan changed files for secrets without printing values.
- [ ] Documentation and full verification: README architecture/rules/deferred
  codes/config/scope/usage/exit semantics; retain warning. Run fmt, all-target
  check, workspace tests, Clippy -D warnings and RustSec. Manually validate every
  demo including 100k outputs / 800k fee BLOCK with TG002 and TG003. Record in
  `docs/milestone-3-verification.md`; mark roadmap only after success.
- [ ] Final scope and history inspection; normal push to origin; verify remote
  HEAD matches. Stop without creating PR #3 or beginning Milestone 4.

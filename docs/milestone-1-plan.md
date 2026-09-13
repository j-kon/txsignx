# Milestone 1 implementation plan

Goal: a reusable, defensive raw Bitcoin transaction analyzer and the
`txsignx tx inspect <RAW_TX_HEX> [--json]` CLI specified in the user brief.

## Design and scope

The inspected base is `d4bec34c78fc7d08841a09f2188d6254fd798276`, aligned with
origin/main. It contains only an empty `crates/txsignx` library. Rename that
scaffold to `txsignx-core` and add `txsignx-cli`; these are the only active crates.
Work on `milestone-1-transaction-analysis`. Do not modify sibling projects,
rewrite history, open a PR, or start Milestone 2.

Use rust-bitcoin's stable 0.32 series for consensus decoding, identifiers,
metrics, and script helpers. Use its hex facilities instead of another hex
crate. Core report values are primitive integers, booleans, strings, and
serializable structures. Presentation belongs exclusively to the CLI.

Reject empty, oversized, odd-length, non-hex, malformed consensus, and trailing
input with typed errors. Accept strict ASCII hex (either case), without trimming,
0x prefixes, or separators. Check an 8,000,000-character bound before decoding
(4,000,000 serialized bytes), then enforce 4,000,000 WU after decoding. These are
upper safety bounds from block limits, not proof of transaction validity.
Sum outputs with checked arithmetic. Never infer network, produce addresses,
calculate fees, or equate explicit RBF with actual mempool replaceability.

## Implementation sequence

- [ ] Workspace: rename the empty crate, add the CLI target, centralize only
  bitcoin/clap/serde/serde_json/thiserror, resolve Cargo.lock, run workspace check.
- [ ] Script classification: in `transaction/script.rs`, test canonical P2PKH,
  P2SH, P2WPKH, P2WSH, P2TR, OP_RETURN, unknown and near-miss scripts; observe
  failures, then implement `classify_script(&bitcoin::Script) -> ScriptType`.
- [ ] Explicit RBF: in `transaction/rbf.rs`, test sequence values 0, 0xfffffffd,
  0xfffffffe and 0xffffffff; implement `signals_explicit_rbf(u32) -> bool`.
- [ ] Parsing: add `error.rs`, `limits.rs` and `transaction/analyzer.rs`. Test
  rejection of empty, odd, invalid, oversize, truncated and trailing inputs;
  implement the guarded rust-bitcoin decode path, then run core tests.
- [ ] Reports: add `transaction/report.rs` and the public
  `analyze_transaction(&str) -> Result<TransactionReport, AnalysisError>` API.
  Use fixed legacy and SegWit fixtures with independently calculated hash and
  size expectations. Test all summary, input, witness, output and JSON fields,
  totals, overflow, weight boundaries and any-input RBF aggregation.
- [ ] Human CLI: implement clap commands in `main.rs` and output in `display.rs`.
  Add real binary integration tests for help, version, legacy and witness output,
  error exit codes and stdout/stderr separation. No ANSI styling.
- [ ] JSON CLI: add `--json`, serialize only the report to stdout, and test with
  serde_json including invalid-input behavior. Handle output I/O errors.
- [ ] Adversarial review: exercise malformed short byte sequences, impossible
  CompactSize lengths, script near-misses, witness allocation and arithmetic
  limits; fix any uncovered issue with a regression test.
- [ ] Documentation and verification: update README capabilities, examples,
  limitations, exact development warning and six-milestone roadmap. Run fmt,
  all-target/all-feature check and Clippy, offline workspace tests and required
  CLI commands. Review diff, dependencies and secret patterns before final
  commit. Mark only Milestone 1 complete once verified, then push normally.

Each behavior is developed with failing tests before implementation, verified,
and committed in natural conventional-commit units. Tests must not require
network, keys or RPC. Literal fixture expectations must not be computed by the
analyzer under test. cargo-audit was checked and is unavailable; do not install it.

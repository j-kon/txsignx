# Milestone 2 verification — PSBT inspection

Verified 2026-09-14 in the Rust repository `j-kon/txsignx`, branch
`milestone-2-psbt-inspection`, based on
`ca65a88c9b2e38995aca086648b9cfe4ecdfbfc4`.
The tested implementation commit is `6985079`; subsequent changes in this
handoff only update documentation. This record's commit and the final pushed
HEAD can be obtained with `git log -1` on the branch.

## Environment and checks

- Rust 1.95.0 (`59807616e`, 2026-04-14), Cargo 1.95.0.
- `cargo fmt --check`: passed.
- `cargo check --workspace --all-targets --all-features`: passed.
- `cargo test --workspace`: **100 passed, 0 failed, 0 ignored**.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo audit` 0.22.2: passed, 50 locked crate dependencies scanned,
  **0 known vulnerabilities, 0 warnings**. Refreshed 1,244-advisory database at
  `455fd4bac659b5f1fca3810661c2d8b3c25dad05`.
- `cargo run -p txsignx-cli -- --help`: passed; tx and psbt commands shown.
- `cargo run -p txsignx-cli -- --version`: passed; `txsignx 0.1.0`.
- `git diff --check`: passed.

There are **46 new Milestone 2 tests**, including two shared-analyzer regression
tests. All **54 Milestone 1 tests remain passing**. The new tests comprise:
2 decoded-transaction tests, 9 core PSBT unit tests, 10 PSBT parsing tests,
7 PSBT adversarial tests, 10 report tests, 1 CLI bounded-reader unit test,
and 7 CLI process tests. Tests use deterministic local data and no network.
The audit alone accesses the advisory database/index.

## Manual fixture verification

For both `fixtures/psbt-unsigned.b64` and `fixtures/psbt-partial.b64`, ran the
compiled binary with positional text, `--file`, and piped `--stdin`, each in
human and JSON modes: **all 12 combinations passed**. JSON was parsed with
Python; all sources produced identical reports and empty stderr.

- Unsigned fixture: `unsigned`, PSBT version 0, BIP174, one input, two outputs,
  150,000 output sats, explicit RBF, 1,000-sat fee from supplied UTXO context.
- Partial fixture: `partially_signed`, one syntactically encoded ECDSA signature,
  explicit SIGHASH_ALL, same transaction facts and 1,000-sat fee.
- Unsigned TXID:
  `a6375ce044efb3b817642d5b3c584e63e6e5de2b32cbaed1a34b72c1156d6381`.
  Independently checked with Python double-SHA256 of the 116-byte unsigned
  transaction and digest reversal.
- Human output identifies structural inspection and unavailable definitive
  feerate. JSON contains only the report, with no network/address/feerate claim.
- Legacy and SegWit raw fixture JSON still parses, preserving null raw fees.

Fixture builders use a public generator point, constant dummy metadata and a
syntactically encoded signature. No private key is constructed or signing
performed; signature validity is not asserted. See `../fixtures/README.md`.

## Security and correctness review

Completed local source review and an independent source-only review of the
implementation and relevant dependency paths. No remaining blocking findings.
This is **not a professional security audit**.

Resolved during review:

1. **TapTree allocation expansion.** Compact tree values can expand into many
   dependency leaf/branch objects. The allocation-free framing preflight now
   limits aggregate output TapTree value bytes to 12,288, allowing at most 4,096
   minimum-sized leaves before dependency parsing. Tests prove rejection above
   the limit, acceptance at the boundary, and aggregation across output maps.
2. **CLI privacy and diagnostics.** Clap's default error formatting can echo
   positional values. Parse failures now use fixed sanitized messages; help and
   version remain static. A final usability correction makes the message neutral
   for raw-transaction argument failures too.

Reviewed protections:

- `rg` found no production `unsafe`, `panic!`, `unwrap`, `expect`, or
  `unreachable!` paths in repository source. Matching unwraps are in test modules.
- Text and decoded limits precede semantic parsing; map/pair limits and unsigned
  transaction/TapTree bounds limit structural expansion. Tests exercise all
  one-/two-byte buffers, fixture mutations/truncation, huge declared lengths,
  duplicate/malformed keys, separators and trailing data.
- An exact 16 MiB decoded PSBT with CRLF text is accepted; one more decoded byte
  is rejected. Large non-global metadata beyond 4 MB is accepted. Dependency
  global-map/per-field bounds still apply separately.
- Non-witness TXID and vout are checked; both UTXO forms must match value and
  script. Invalid context exposes no resolved output facts and suppresses fees.
- rust-bitcoin fee arithmetic runs only after map-count and consistency checks.
  Missing/invalid/negative/overflow states carry null fee values. Unsigned output
  sum overflow remains the shared typed transaction-analysis error.
- All signing classifications are field-presence facts. Empty finalization
  markers take precedence. No signature validation or broadcast-readiness claim.
- Upstream PSBT errors can contain raw metadata; boundary errors retain neither
  those values nor their error sources. Report output uses numeric/boolean facts,
  controlled enum strings, hashes and lowercase script hex.
- Unknown/proprietary fields are preserved in parsed PSBTs and counted in reports;
  arbitrary values, xpub strings, public keys, origins and signatures are not
  dumped. Reports still reveal scripts/transaction graphs and need privacy care.
- No network inference, address generation, ownership claim, definitive feerate,
  signing, finalizing, extraction, broadcasting, node/wallet I/O, or policy rules.

## Dependencies and scope

No direct runtime dependencies or existing package versions changed.
`bitcoin` remains 0.32.102; enabling its existing `base64` feature adds only the
optional transitive `base64` 0.21.7 package to Cargo.lock. No versions or advisory
ignores were changed to obtain a clean audit.

A tracked-file pattern scan found no private-key PEM blocks, extended private
keys, GitHub tokens, AWS access keys, assigned credential patterns, `.env`
files, or wallet/database artifacts. Fixture construction and changed-file
review found only public dummy data. Pattern scans are not proof that every
possible secret format is absent.

Only this Rust repository changed. Sibling web/docs/brand workspaces and other
projects were untouched. No reset, rebase, clean, force push, amend, squash or
history rewrite occurred. No PR was opened or merged. No Milestone 3 policy
engine was started. Existing CI configuration was preserved; it runs on main
pushes and main-targeting PRs, so a feature-branch-only push does not trigger it.

The final handoff is a normal `git push -u origin milestone-2-psbt-inspection`
after committing this record; remote HEAD equality and push status are checked
and reported in the session. No PR follows the push.

## Changed files

- `Cargo.lock`
- `Cargo.toml`
- `README.md`
- `crates/txsignx-cli/src/display.rs`
- `crates/txsignx-cli/src/main.rs`
- `crates/txsignx-cli/src/psbt_display.rs`
- `crates/txsignx-cli/src/psbt_input.rs`
- `crates/txsignx-cli/tests/psbt_cli.rs`
- `crates/txsignx-core/examples/psbt_fixtures.rs`
- `crates/txsignx-core/src/lib.rs`
- `crates/txsignx-core/src/limits.rs`
- `crates/txsignx-core/src/psbt/analyzer.rs`
- `crates/txsignx-core/src/psbt/error.rs`
- `crates/txsignx-core/src/psbt/fee.rs`
- `crates/txsignx-core/src/psbt/framing.rs`
- `crates/txsignx-core/src/psbt/mod.rs`
- `crates/txsignx-core/src/psbt/parser.rs`
- `crates/txsignx-core/src/psbt/report.rs`
- `crates/txsignx-core/src/psbt/signing.rs`
- `crates/txsignx-core/src/psbt/utxo.rs`
- `crates/txsignx-core/src/transaction/analyzer.rs`
- `crates/txsignx-core/src/transaction/mod.rs`
- `crates/txsignx-core/tests/decoded_transaction.rs`
- `crates/txsignx-core/tests/psbt_adversarial.rs`
- `crates/txsignx-core/tests/psbt_common/mod.rs`
- `crates/txsignx-core/tests/psbt_parsing.rs`
- `crates/txsignx-core/tests/psbt_report.rs`
- `docs/milestone-2-plan.md`
- `docs/milestone-2-verification.md`
- `fixtures/README.md`
- `fixtures/psbt-partial.b64`
- `fixtures/psbt-unsigned.b64`

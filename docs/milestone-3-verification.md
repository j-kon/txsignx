# Milestone 3 verification — deterministic security policy engine

Verified 2026-09-14 in `j-kon/txsignx`, branch `milestone-3-policy-engine`.
Base: `4fcde9df8c523d58dc19700534dea43c78e4a776`.
Full checks ran at `d0794cf5897daccc48cddf32c2a2b11958795fe9`; the final
verification commit only adds this record, updates plan checkboxes and marks
Milestone 3 complete in the README roadmap. Obtain the final pushed HEAD with
`git log -1` on the branch; remote equality is verified in the session handoff.

## Architecture and scope

A new workspace crate, `txsignx-policy`, reads immutable `PsbtReport` facts and
returns `PolicyReport`. Core derives facts; policy interprets them deterministically;
CLI presents inspection and policy together. Core has no policy dependency.
The policy crate's runtime dependencies are only existing `serde` and
`txsignx-core`; `serde_json` is test-only. CLI adds the local policy dependency.
**No new crates.io packages, package upgrades or external checksum changes**
were introduced. Cargo.lock changes reflect only workspace dependency edges
and the new local package.

`PolicyReport` contains decision, risk, highest severity, count, ordered findings,
evaluated codes, applied configuration and a scope note. Findings include code,
severity, title, message, recommendation and tagged global/input/output location.
Rule metadata powers CLI listing. Reserved metadata has no evaluator/severity.
Duplicate registration and mismatched finding codes are rejected. Registry and
location ordering use Vec/BTreeSet and deterministic sorting, never HashMap order.

Decision mapping: Critical => BLOCK; High/Medium => REVIEW; Info/Low/none => PASS.
Risk follows the highest severity, mapping Info/Low/none to Low. No accumulated
or probabilistic score exists. PASS is not a universal signing permission.
Development defaults are 100,000 absolute fee sats and 1,000 bps fee/input share.
Configuration validates bps <=10,000, with zero allowed; equality does not trigger.
Fee-share addition and multiplication widen to u128 before arithmetic.

Active rules: TG002 absolute fee (Critical), TG003 input-value fee share (Critical),
TG009 invalid supplied UTXO context (Critical), TG010 missing context (High),
TG011 unusual explicit numeric sighash (High), TG012 extension metadata counts
(Info), TG013 unrecognized output/resolved-prevout templates (Medium), and TG014
positive OP_RETURN value (Critical). TG001/TG004/TG005/TG006/TG007/TG008 are
reserved/deferred and never generate findings.

Missing/invalid UTXO fee states skip fee rules and remain visible via TG010/TG009.
Negative/overflow/other fee failures return typed evaluation errors, not PASS.
Inconsistent fee-state/value/context and map-count combinations also fail.
Direct custom rules are trusted application extensions; callers should use the
engine for validated configuration and report-boundary checks.

## Full verification

Environment: Rust 1.95.0 (`59807616e`, 2026-04-14), Cargo 1.95.0,
cargo-audit 0.22.2.

| Check | Result |
|---|---|
| `cargo fmt --check` | Passed |
| `cargo check --workspace --all-targets --all-features` | Passed |
| `cargo test --workspace` | **167 passed, 0 failed, 0 ignored** |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Passed |
| `cargo audit` | **0 known vulnerabilities, 0 warnings** |
| CLI `--help`, `--version` | Passed; version remains 0.1.0 |
| `policy list`, `policy list --json` | Passed; 8 active, 6 deferred |
| `git diff --check` | Passed |

RustSec refreshed 1,244 advisories at database commit
`455fd4bac659b5f1fca3810661c2d8b3c25dad05`, scanning 51 locked crate dependencies.
No advisories were ignored and no dependency versions were changed to silence
an audit. Test/evaluation code uses no network; audit and Git operations do.

All **100 Milestone 1/2 tests remain passing**. **67 new Milestone 3 tests**:
55 policy tests (3 models, 13 engine, 8 fee, 6 context/sighash, 7 metadata/script,
2 registry, 11 fixtures, 5 adversarial) and 12 CLI integration tests.
Coverage includes all severity mappings, strict boundaries, widened u64 extremes,
zero denominator/threshold, standard and unusual sighashes, each UTXO status,
read-only evaluation, duplicate IDs, registry/location order, repeatable JSON,
metadata suppression, bounded input reuse and intentional exit codes.

## Manual demonstrations

Ran existing unsigned and partially signed fixtures through `cargo run` preflight:
both PASS. Ran `policy list` in human and JSON modes; JSON parsed with Python.
Then ran each of the ten evaluable policy fixtures with positional, `--file`
and `--stdin` sources, each in human and JSON modes: **all 60 combinations passed**.
For every fixture the three JSON sources matched byte-for-byte, stderr was empty,
and decision/risk/codes/exit status matched the table below.

| Fixture | Decision / risk | Findings | Exit |
|---|---|---|---|
| pass | PASS / Low | None | 0 |
| absolute-fee | BLOCK / Critical | TG002 | 3 |
| percentage-fee | BLOCK / Critical | TG003 | 3 |
| 800k-fee | BLOCK / Critical | TG002 + TG003 | 3 |
| missing-utxo | REVIEW / High | TG010 | 2 |
| invalid-utxo | BLOCK / Critical | TG009 | 3 |
| unusual-sighash | REVIEW / High | TG011 | 2 |
| op-return | BLOCK / Critical | TG014 | 3 |
| unknown-script | REVIEW / Medium | TG013 | 2 |
| extension-metadata | PASS / Low | TG012 | 0 |

The capstone fixture reports exactly **100,000 output sats**, **800,000 fee sats**,
and **900,000 total input sats**. Both findings are Critical. Human output shows
BLOCK / CRITICAL and both codes; parsed JSON asserts the exact amounts/severities.

The additional negative-fee fixture is tested as an evaluation error: exit 1,
empty stdout, sanitized stderr. Input/config/runtime errors use 1; successful
preflight REVIEW/BLOCK emit complete JSON before returning 2/3. Argument errors
now use 1 consistently (prior inspect Clap errors used 2); successful inspect
output/JSON and runtime semantics remain unchanged.

Legacy and SegWit raw JSON regressions passed with null raw fees. Policy BLOCK
does not alter independent `psbt inspect` behavior. Threshold override tests
cover exact absolute boundaries, zero/66/67/10,000 bps, and invalid bps.
All eleven fixture files were regenerated and compared byte-for-byte with the
checked-in files. See `../fixtures/policy/README.md` for construction details.

## Security and correctness review

An independent source review found **no blocking issues** and ran bounded offline
policy/CLI tests. Local review and additional adversarial tests checked:

- No production unsafe, panic, unwrap/expect, floating-point comparisons,
  nondeterministic HashMap iteration, random/time dependence, or network I/O in
  the new policy paths. `rg` scans of policy and CLI source found none.
- u128 fee comparisons widen before both addition and multiplication. Boundary
  tests cover u64 maxima, exact limits, zero values and a small rational grid.
- Missing and invalid UTXO contexts do not fabricate fee findings or become PASS
  under the active registry. Other fee failures stop evaluation explicitly.
- Immutable report references; tests compare reports before/after evaluation.
  Per-rule ordering and repeated JSON remain deterministic, including permuted
  input/output vectors with stable indices.
- No raw PSBT data, signature bytes, xpubs, key origins, arbitrary scripts or
  metadata strings enter policy messages. Explicit sighash display strings are
  ignored. A public dummy extension containing a terminal escape and sentinel
  is counted without reproducing its contents in human or JSON output.
- Preflight uses the existing bounded input reader. Malformed/oversized input,
  source conflicts and invalid configuration yield sanitized errors and no JSON
  report. Decision exits happen only after output flush.
- No fabricated network, ownership, change, confirmation, signature-validity,
  definitive feerate or universal signing-safety claims. Deferred rules remain
  context descriptions, not approximations. Scope note appears in policy output.

No private keys, seed phrases, wallet databases or credentials were used in the
fixtures. A tracked-file pattern scan found no private-key PEM, extended private
key, GitHub/AWS token or assigned-credential patterns, `.env` files, or wallet/DB
artifacts. Findings report paths/categories only, not candidate secret values.
Pattern scans do not establish the absence of every possible secret format.
This work is **not a professional security audit**.

## Git scope and handoff

Only this Rust repository changed. No Milestone 4 work, wallet/node integration,
PR #3 or merge was performed. No reset, rebase, clean, force push, amend, squash
or history rewrite occurred. Sibling web/docs/brand and unrelated repositories
were untouched. Existing milestone branches remain preserved.

Final handoff: commit this record, inspect clean status/history/scope, then
`git push -u origin milestone-3-policy-engine` normally. Remote HEAD equality
and final push status are checked and reported in the session. The existing CI
workflow runs on main pushes and main-targeting PRs, not this branch-only push.
No PR is opened as part of the handoff.

## Changed files

- `Cargo.lock`
- `Cargo.toml`
- `README.md`
- `crates/txsignx-cli/Cargo.toml`
- `crates/txsignx-cli/src/main.rs`
- `crates/txsignx-cli/src/preflight.rs`
- `crates/txsignx-cli/tests/preflight.rs`
- `crates/txsignx-core/examples/policy_fixtures.rs`
- `crates/txsignx-policy/Cargo.toml`
- `crates/txsignx-policy/src/config.rs`
- `crates/txsignx-policy/src/engine.rs`
- `crates/txsignx-policy/src/error.rs`
- `crates/txsignx-policy/src/lib.rs`
- `crates/txsignx-policy/src/model.rs`
- `crates/txsignx-policy/src/registry.rs`
- `crates/txsignx-policy/src/rules/fees.rs`
- `crates/txsignx-policy/src/rules/metadata.rs`
- `crates/txsignx-policy/src/rules/mod.rs`
- `crates/txsignx-policy/src/rules/scripts.rs`
- `crates/txsignx-policy/src/rules/sighash.rs`
- `crates/txsignx-policy/src/rules/utxo.rs`
- `crates/txsignx-policy/tests/adversarial.rs`
- `crates/txsignx-policy/tests/common/mod.rs`
- `crates/txsignx-policy/tests/context_rules.rs`
- `crates/txsignx-policy/tests/engine.rs`
- `crates/txsignx-policy/tests/fees.rs`
- `crates/txsignx-policy/tests/fixtures.rs`
- `crates/txsignx-policy/tests/metadata_scripts.rs`
- `crates/txsignx-policy/tests/models.rs`
- `crates/txsignx-policy/tests/registry.rs`
- `docs/milestone-3-plan.md`
- `docs/milestone-3-verification.md`
- `fixtures/policy/800k-fee.b64`
- `fixtures/policy/README.md`
- `fixtures/policy/absolute-fee.b64`
- `fixtures/policy/extension-metadata.b64`
- `fixtures/policy/invalid-utxo.b64`
- `fixtures/policy/missing-utxo.b64`
- `fixtures/policy/negative-fee.b64`
- `fixtures/policy/op-return.b64`
- `fixtures/policy/pass.b64`
- `fixtures/policy/percentage-fee.b64`
- `fixtures/policy/unknown-script.b64`
- `fixtures/policy/unusual-sighash.b64`

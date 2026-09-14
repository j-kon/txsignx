# Milestone 4 verification

Verified 2026-09-14. This is an implementation verification and internal code
review, **not a professional security audit**.

## Repository and baseline

- Repository: `~/Developer/jaykon/txsignx/txsignx` (`j-kon/txsignx`).
- Branch: `milestone-4-wallet-context`.
- Required main/base: `82edd04c35ac2a85323495f62e53c0eafd366ead`.
- Started clean, 0 commits ahead, with no remote Milestone 4 branch.
- HEAD/main/origin/main/merge-base matched the required SHA before implementation.
- Baseline fmt/check/test/Clippy/audit passed: 167 tests, 8 active/6 deferred rules.
- Baseline main CI run 34852960114 was confirmed successful for the base SHA.
- Final verified code revision: `e4e6a378f9f8e70556fbba0c60bfd81acadfdfcf`.
  The following completion commit contains documentation only.

## Architecture and descriptor boundary

Core remains unchanged and independent of BDK, wallet and policy. The new
`txsignx-wallet` depends on core and BDK public descriptor functionality. Policy
consumes read-only wallet context; CLI orchestrates inputs, classification and
presentation. No BDK Wallet, signer, chain backend or persistence object is used.

`Descriptor<DescriptorPublicKey>::from_str` is the structural public-only parsing
boundary. Neither secret-accepting `parse_descriptor` nor BDK string descriptor
conversion is called. Public descriptors are passed as typed objects to BDK's
network-kind checker; the returned key map must be empty. No descriptor-bearing
configuration/index Debug or Serialize implementation is exposed.

External and internal descriptors must be ranged, single-path, distinct and
publicly derivable. Hardened suffixes/wildcards are rejected; hardened origin
metadata is permitted. All derived script duplicates fail with typed errors,
including overlapping descriptors differing only in origin metadata.

Configured networks: bitcoin (mainnet alias), testnet, testnet4, signet, regtest.
Serialization uses canonical names. BDK validates main/test extended-key
compatibility; test-family prefixes cannot distinguish their networks. No
transaction/PSBT/script network inference occurs, and TG001 remains deferred.

Derivation is exactly `0..window` on both keychains. Default 1000, maximum 10000,
minimum 1. Additional limits: 64 KiB per descriptor, 200,000 aggregate key/path
work units and 16 MiB derived script bytes. PSBT contents cannot expand the range.
The private BTreeMap has deterministic key lookup; reports use ordered vectors.

Valid core UTXO facts classify as External/Internal with derivation index or
NoMatchWithinWindow. Missing/invalid prevout context stays Unavailable with an
explicit reason. Outputs are classified from their script bytes without change
heuristics. A descriptor miss is bounded evidence, not proof of non-ownership.

Expected change is explicit, zero-based caller intent. Out-of-range indexes and
duplicates are rejected; accepted indexes are sorted. Wallet reports are privately
constructed, read-only through getters, and bound to the inspection transaction
ID, ordered scripts and UTXO statuses. Policy rejects mismatched context reuse.

## Policy and compatibility

TG004 triggers only for valid resolved unmatched inputs: HIGH/REVIEW. Missing
and invalid inputs retain TG010/TG009 findings without duplicate TG004 findings.
Collaborative transactions can legitimately contain unmatched inputs.

TG005 only considers explicitly declared change: internal match produces no
finding; external match is HIGH/REVIEW; no match is CRITICAL/BLOCK. With no
expected-change intent or no wallet context it is not evaluated.

`rule_evaluations` records evaluated, partially_evaluated or not_evaluated, with
typed reasons. TG004 is partial with mixed usable/unavailable inputs and skipped
with no usable context. `evaluated_rules` is retained for rules that ran fully or
partially. No-context preflight still works; optional `wallet_context` is omitted.
Scope notes explain missing wallet context and bounded checks. PASS never implies
that skipped wallet rules, chain state or network truth were verified.

Active: TG002/TG003/TG004/TG005/TG009/TG010/TG011/TG012/TG013/TG014 (10).
Deferred: TG001/TG006/TG007/TG008 (4).

## Automated verification

Toolchain: rustc 1.95.0, cargo 1.95.0, cargo-audit 0.22.2.

| Command | Result |
|---|---|
| cargo fmt --check | Passed |
| cargo check --workspace --all-targets --all-features | Passed |
| cargo test --workspace | 224 passed; 0 failed |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | Passed |
| cargo audit | Passed; 0 known vulnerabilities; 0 warnings |
| cargo tree / cargo tree --duplicates | Reviewed; one Bitcoin version |
| git diff --check | Passed |
| CLI help / version / policy list / policy list --json | Passed; 10 active / 4 deferred |

The original 167 tests remain, with intentional registry count/order changes,
explicit None context initialization and optional report-field initialization.
No previous regression coverage was removed. There are **57 new M4 tests**:
24 wallet tests, 14 policy tests, and 19 CLI/reader tests. Total by component:
core 83, wallet 24, policy 69, CLI 48 = 224.

Coverage includes public/network/checksum/ranged/multipath/private-key rejection,
window min/default/max, half-open boundaries, overlapping mappings, missing and
invalid UTXOs, partial/skipped evaluations, collaborative inputs, multiple change
outputs, duplicates/out-of-range intent, report binding, determinism, Taproot and
multisig descriptors, descriptor size/work bounds, and privacy-safe CLI errors.

## Manual demonstrations and regressions

Deterministic fixtures were generated using a public curve point and dummy chain
code, without a seed or private key. Generator output was compared byte-for-byte
with all checked-in public descriptor and PSBT fixtures.

| Fixture/check | Observed result |
|---|---|
| wallet/payment | PASS/0; external input 7, internal change 3 |
| wallet/foreign-input | TG004 HIGH REVIEW/2 |
| wallet/external-change | TG005 HIGH REVIEW/2 |
| wallet/change-hijack | TG005 CRITICAL BLOCK/3 |
| wallet/collaborative | TG004 HIGH REVIEW/2 |
| wallet/missing-utxo | TG010 REVIEW/2; missing unavailable input; no TG004 |
| wallet/invalid-utxo | TG009 BLOCK/3; invalid unavailable input; no TG004 |
| wallet/boundary-999 at window 1000 | Match, PASS/0 |
| wallet/boundary-1000 at window 1000 | NoMatchWithinWindow, TG004 REVIEW/2 |
| wallet/boundary-1000 at window 1001 | Match, PASS/0 |
| No expected-change flag | TG005 not_evaluated |
| Positional / --file / --stdin wallet input | Passed; human output and matching JSON |
| Descriptor direct/file input | Equivalent results; same-keychain conflicts rejected |
| JSON privacy | No supplied descriptor or extended-public-key encoding in reports |
| M3 pass / extension-metadata | PASS/0 (TG012 retained for extension metadata) |
| M3 800k-fee | BLOCK/3, TG002 + TG003 retained |
| M3 missing/invalid/unusual-sighash/OP_RETURN/unknown-script | Original decisions/exits retained |
| Legacy and SegWit raw inspection | Passed |
| Unsigned and partially signed PSBT inspection | Passed |

The manual privacy check compares actual supplied encodings, not the substring
`xpub` in the existing factual JSON field `global_xpub_count`. Transaction ID
substrings likewise are not key-origin disclosures.

## Dependency and supply-chain review

BDK Wallet **3.1.0**, only std enabled, requires Bitcoin **^0.32.8** and miniscript
**^12.3.5**. The tree resolves Bitcoin **0.32.102** throughout and miniscript
**12.3.7**, without serialization adapters, compatibility crates or duplicate
Bitcoin type universes. Existing dependency versions are unchanged.

Cargo.lock grows from 51 to 71 packages: 20 added, 0 removed. This comprises
1 local wallet package, BDK Wallet itself, and 18 external transitive packages.
The added transitives are bdk_chain 0.23.3, bdk_core 0.6.3, miniscript 12.3.7,
rand 0.8.8, rand_chacha 0.3.1, rand_core 0.6.4, getrandom 0.2.17, libc 0.2.189,
ppv-lite86 0.2.21, zerocopy/zerocopy-derive 0.8.57, cfg-if 1.0.4, wasi
0.11.1+wasi-snapshot-preview1, ahash 0.8.12, hashbrown 0.14.5, once_cell 1.21.4,
version_check 0.9.5, and syn 2.0.119. Lockfile includes optional/target-dependent
resolution entries; not every entry is built for the native target. Native
`cargo tree --duplicates` reports none. No file-store, SQLite, network-client or
mnemonic feature is enabled.

RustSec scan loaded 1246 advisories, database revision
`e2e640471715167f73e22eaf761f2e547adafeec`, and scanned 71 packages successfully.
No advisory was suppressed and no existing version changed to obtain a clean audit.

## Internal security review and privacy scan

Two dependency panic paths were exposed by adversarial tests and removed from
the application path: hardened public derivation and BIP32 depth exhaustion.
Hardened derivation is rejected structurally; a fallible public-key translator
propagates all rust-bitcoin public derivation errors instead of entering
miniscript's definite-key unreachable assumptions. Regression tests now pass.
A final intent check also distinguishes out-of-range from duplicate change indexes.

Reviewed new production paths for unsafe, panic/unwrap/expect/todo/unreachable,
overflow, resource expansion, descriptor/key-origin leakage, arbitrary logging,
network/persistence calls, false ownership/network/balance claims, duplicate UTXO
findings, heuristic change inference and nondeterministic report ordering.
No remaining issue was identified in this review. Allocation failure and unknown
dependency defects are not ruled out by these tests.

Tracked-change scan found no literal extended private keys, WIF secrets, private
PEM blocks, seed phrases, mnemonic material, credentials, API keys, .env files or
wallet databases. The only intentional private-key construction is generated
dummy material in `crates/txsignx-wallet/tests/config.rs` for rejection tests;
no private fixture files are committed. Public tpub fixtures are intentional
demo data. Malformed direct/file descriptor sentinels never appear on stdout or
stderr; valid reports omit descriptors, encoded xpubs, origins and checksums.

## Scope and handoff

Only the Rust repository changed. Milestone 1/2/3 branches remain preserved
locally/remotely and reachable from main. Work uses normal incremental commits
and a normal branch push only; no reset, clean, rebase, squash, force push,
history rewrite or branch deletion occurred. Sibling web/docs/brand workspaces
were not touched.

No PR #4 is opened by this task. No Milestone 5, Bitcoin Core/RPC/runtime network
I/O, persistence/database, signing, finalizing, broadcasting, balances or chain
state claims are introduced. Builds and RustSec checks may fetch dependencies
and advisory data; application execution does not contact a network.

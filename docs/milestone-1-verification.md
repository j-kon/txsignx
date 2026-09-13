# Milestone 1 verification record

Branch: `milestone-1-transaction-analysis`.
Base: `d4bec34c78fc7d08841a09f2188d6254fd798276`.
Implementation and regression-test tip: `ca93e00` (documentation follows).
Toolchain: Rust/Cargo 1.95.0.

## Checks

- `cargo fmt --check`: passed.
- `cargo check --workspace --all-targets --all-features`: passed.
- `cargo test --workspace`: passed; final `cargo test --workspace --offline`
  passed **54 tests**, zero failures (9 CLI, 9 script/RBF, 12 parsing,
  15 reports, 9 adversarial).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- CLI `--help`, `--version`, human legacy and human SegWit inspection: passed.
- Both fixture JSON reports parsed with Python and jq. Metrics, integer amounts,
  null fee, absent network, and independently computed SHA-256 hashes matched.

| Fixture | Bytes | Weight | Vsize | Witness |
| --- | ---: | ---: | ---: | --- |
| Legacy | 118 | 472 WU | 118 vB | No |
| SegWit | 129 | 483 WU | 121 vB | Yes |

Both report 150,000 output satoshis and explicit RBF signaling. Both have TXID
`15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6`.
Legacy wTXID equals TXID; SegWit wTXID is
`561d35cd60944685cbc9155bb5ea54de63aa4ec39c4ac3f2aa936f127cbeccd1`.
The known genesis transaction additionally verifies its published TXID,
204-byte size, 816 WU and 5,000,000,000 output satoshis.

## Security review

Implementation review and an independent read-only code review found no
blocking defects. Documentation was independently checked against the code.
This is not a professional security audit.

- No panic/unwrap/expect on the production untrusted-input path. Test fixtures
  use unwrap only to enforce test expectations. Malformed input produces typed
  errors; all one/two-byte inputs and deterministic short buffers are covered.
- Hex size is checked before decode allocation. rust-bitcoin performs consensus
  decoding and full-consumption checks; trailing bytes are rejected.
- 4,000,000 serialized bytes / 8,000,000 hex characters and 4,000,000 WU provide
  upper bounds. Report construction checks a transaction-wide 100,000 witness
  item ceiling before allocating witness report objects. Tests cover exact
  boundaries, aggregate limits, huge declared lengths and truncation.
- Output totals use checked arithmetic. Maximum-u64 JSON round trips preserve
  integer precision; monetary consensus validity is explicitly outside scope.
- Canonical script helpers are used. Witness/scripts are rendered as hex,
  preventing script-controlled terminal text from being interpreted.
- No inferred network/address, computed fee/feerate, or claim of definitive
  mempool replaceability. `fee_sats` is null without prevout context.
- Analysis errors leave stdout empty; JSON is streamed to a buffered writer.
  Output I/O errors propagate. A closed-stdout check exited nonzero without panic.
- No logging, RPC, wallet, storage, signing, or outbound requests in production.
  Reports intentionally reproduce transaction script/witness data: treat report
  sharing and shell argument history accordingly.
- Direct dependencies are bitcoin 0.32.102 (std only), clap 4.6.6 (derive),
  serde 1.0.229 (derive), serde_json 1.0.151, thiserror 2.0.20, plus the internal
  txsignx-core path dependency. Cargo.lock fixes resolved versions. No standalone
  hex dependency, wallet/node/database/API/async crates were added.
- `cargo audit --version` reported no installed audit command. It was not
  installed. No RustSec vulnerability-database audit was performed.

## Scope

Only this Rust repository was modified. The empty original `crates/txsignx`
scaffold was replaced by the two requested active crates, `txsignx-core` and
`txsignx-cli`. No sibling web/docs/brand files or unrelated repositories were
modified. No private keys, real signatures, seed phrases, mnemonics, xprv,
credentials, environment files, wallet databases or API tokens were introduced.
Fixtures are public dummy data and the public genesis transaction.

Milestone 1 is complete. Milestones 2–6 remain unimplemented. Final branch
publication uses a normal push only; no PR or history rewriting is authorized.

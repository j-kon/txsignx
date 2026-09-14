# TxSignX

**Inspect. Verify. Sign with Confidence.**

Bitcoin transaction security before signing.

TxSignX is an open-source Bitcoin transaction and PSBT security preflight engine
written in Rust. Milestone 1 implements raw-transaction inspection; Milestone 2
adds PSBT v0 inspection. Context-aware policy analysis is planned for later milestones.

> TxSignX is under active development and is currently intended for development and Regtest testing. Do not rely on it to protect real Bitcoin funds.

## Milestone 1 — Raw transaction analysis

The reusable `txsignx-core` library decodes real Bitcoin consensus transaction
serialization with rust-bitcoin. `txsignx-cli` provides the `txsignx` binary.

Reports contain:

- TXID, wTXID, version, raw locktime, input/output counts, serialized byte size,
  weight in WU, virtual size in vB, witness presence, explicit RBF signaling,
  and checked total output value in integer satoshis.
- Each input's index, previous TXID/vout, sequence, scriptSig hex and byte length,
  explicit RBF signal, witness item count, and ordered witness items (index,
  byte length, lowercase hex). Empty witness items are preserved.
- Each output's index, integer satoshi value, scriptPubKey hex and byte length,
  and structural script classification.

Supported script types are **P2PKH, P2SH, P2WPKH, P2WSH, P2TR, OP_RETURN** and
**Unknown**. Classification uses rust-bitcoin's script helpers. Other script
forms, including bare P2PK, are currently Unknown. Classification does not
execute a script, validate a key, or prove an output can be spent.

## Build and use

Install stable Rust with rustfmt and Clippy. Verification used Rust/Cargo 1.95.0.
Run from this Rust workspace, `txsignx/txsignx/`:

```sh
cargo build -p txsignx-cli
./target/debug/txsignx --help
./target/debug/txsignx --version
./target/debug/txsignx tx inspect <RAW_TX_HEX>
./target/debug/txsignx tx inspect <RAW_TX_HEX> --json
```

Replace `<RAW_TX_HEX>` with your transaction. Runnable examples using the checked-in
public dummy fixtures:

```sh
LEGACY_TX=$(cat crates/txsignx-core/tests/fixtures/legacy.hex)
SEGWIT_TX=$(cat crates/txsignx-core/tests/fixtures/segwit.hex)
cargo run -p txsignx-cli -- tx inspect "$LEGACY_TX"
cargo run -p txsignx-cli -- tx inspect "$SEGWIT_TX"
cargo run -p txsignx-cli -- tx inspect "$SEGWIT_TX" --json
```

To install the binary on your Cargo bin path:

```sh
cargo install --path crates/txsignx-cli --locked
txsignx tx inspect "$LEGACY_TX" --json
```

Inputs must be contiguous ASCII hex, in either case. Empty input, whitespace,
`0x` prefixes, odd length, invalid hex, truncated consensus data, and trailing
bytes are rejected with typed core errors. Nothing is silently trimmed or
truncated. Command-line argument length is additionally constrained by the OS;
raw transaction input remains positional; PSBT input also supports file/stdin.

## JSON and library API

`--json` writes only the report to stdout. Errors go to stderr with a nonzero
exit status. Use `cargo run --quiet` to suppress Cargo's own stderr progress
messages, or invoke the compiled binary directly. Validate JSON with:

```sh
./target/debug/txsignx tx inspect "$SEGWIT_TX" --json | python3 -m json.tool
```

JSON uses deterministic snake_case fields, TXID strings, lowercase script and
witness hex, integer sequences and integer satoshi amounts. `fee_sats` is always
`null` in raw-only analysis. There is no network or address field. Consumers
must retain integer precision when parsing satoshi amounts and identifiers.

The core has no CLI formatting or I/O requirements:

```rust
use txsignx_core::{analyze_transaction, AnalysisError, TransactionReport};

fn inspect(raw_hex: &str) -> Result<TransactionReport, AnalysisError> {
    analyze_transaction(raw_hex)
}
```

`transaction::decode_transaction` is also available for bounded decoding into
a rust-bitcoin `Transaction`. The report API maps observations into primitive
values and serializable structures independently of rust-bitcoin's JSON types.

## Context and safety limits

**No network inference.** A raw transaction has no mainnet/testnet/signet/regtest
identifier. The analyzer does not infer a network or generate addresses.

**No raw-transaction fee calculation.** Inputs identify the previous outputs being spent, but
omit their values. Output totals alone cannot establish a fee or feerate.
Human output states `Fee: unavailable without prevout context`; JSON uses
`fee_sats: null`. PSBT inspection can use supplied prevout information for an absolute fee.
Wallet UTXOs and Bitcoin Core context remain future work.

**Explicit RBF only.** Under [BIP125](https://bips.dev/125/), an input signals
explicitly when `nSequence < 0xfffffffe`; the transaction signals if any input
does. Neither `0xfffffffe` nor `0xffffffff` signals explicitly. Inherited
signaling and actual node/mempool replacement policy require external context.
A report saying `Explicit RBF signaling: no` does not establish that a
transaction cannot be replaced.

**Bounded work.** Hex input is capped at 8,000,000 characters before decoding,
corresponding to 4,000,000 serialized bytes. Decoded transactions above 4,000,000
WU are rejected. These generous ceilings follow the
[BIP141 block-weight bound](https://bips.dev/141/); fitting the bound does not
establish that a transaction fits in a valid block or meets relay policy.

Structured reports additionally permit at most **100,000 witness items across
all inputs**. Empty items cost very few serialized bytes but expand into
individual report objects. This implementation resource limit is checked
before report construction, is not a consensus rule, and may reject unusually
large but consensus-decodable witnesses. `decode_transaction` enforces byte and
weight limits but does not build a report or apply this additional item limit.
No input is silently truncated. rust-bitcoin supplies consensus decoding and
its own defensive vector-allocation checks; TxSignX does not implement a second
consensus parser. Output totals use checked addition and reject `u64` overflow.

**Observation, not validation.** Successful decoding is not proof of full
consensus validity, standardness, correct signatures, available UTXOs, valid
amount ranges, finality, wallet ownership, absence of double spending, or safety
to sign. In particular, decoded amounts can exceed Bitcoin's money supply;
Milestone 1 reports them without claiming monetary validity. There is no script
execution, signing, private-key handling, live RPC, wallet, policy
engine, database, server, or web integration in this milestone.

Raw scripts and witness data can contain identifying or sensitive data. The
requested report reproduces those bytes as hex. Review reports before sharing;
command-line arguments may also be visible in shell history or process listings.
TxSignX adds no transaction logging or network transmission. This milestone has
received implementation review and automated tests, not a professional audit.

## Milestone 2 — PSBT inspection

Supports **PSBT v0 / BIP174** using rust-bitcoin 0.32.102. PSBT v2 / BIP370
is unsupported and returns a typed unsupported-version error; it is a possible
future capability. No alternate PSBT parser or direct runtime dependency was
added. The existing bitcoin dependency enables its `base64` feature, adding
only the optional transitive base64 0.21.7 crate.

```sh
txsignx psbt inspect '<BASE64_PSBT>'
txsignx psbt inspect '<BASE64_PSBT>' --json
txsignx psbt inspect --file payment.psbt
txsignx psbt inspect --file payment.psbt --json
cat payment.psbt | txsignx psbt inspect --stdin
cat payment.psbt | txsignx psbt inspect --stdin --json
```

Choose exactly one source. All sources accept standard padded base64 text;
surrounding ASCII whitespace is trimmed, interior whitespace is rejected.
Binary PSBT, hex and URL-safe base64 are not auto-detected. File/stdin reads are
bounded and reject excess data without silent truncation. Prefer these sources
to keep the PSBT out of shell history and process arguments. Runnable dummy
examples use `fixtures/psbt-unsigned.b64` or `fixtures/psbt-partial.b64`:

```sh
./target/debug/txsignx psbt inspect --file fixtures/psbt-unsigned.b64
./target/debug/txsignx psbt inspect --file fixtures/psbt-partial.b64 --json | python3 -m json.tool
```

Library entry points are `txsignx_core::analyze_psbt(&str) ->
Result<PsbtReport, psbt::PsbtError>` and `psbt::decode_psbt(&str)` for an intact
rust-bitcoin PSBT. `transaction::analyze_decoded_transaction(&Transaction)`
shares the existing raw-transaction analysis with PSBT inspection. Raw report
fields and JSON remain unchanged.

PSBT JSON contains primitive values with snake_case field names. It includes
version/format, unsigned TXID, transaction version/locktime, input/output
counts, total output sats, explicit RBF, global xpub/unknown/proprietary counts,
`signing_state`, `fee`, `inputs` and `outputs`. Input records include outpoint,
sequence/RBF, UTXO source/consistency and resolved value/script facts, explicit
sighash numeric value/name (or null), ECDSA/Taproot signature counts/presence,
key-origin counts, script/final-marker presence and unknown/proprietary counts.
Outputs include the same value/script facts as raw analysis plus origin counts,
script presence, Taproot internal-key/tree presence and metadata counts.

**UTXO consistency.** Each input reports `missing`, `witness_utxo`,
`non_witness_utxo`, or `both` as its source. Status is `missing`, `valid`,
`txid_mismatch`, `vout_out_of_range`, or `witness_non_witness_mismatch`.
A supplied previous transaction must match the outpoint TXID and contain its
vout. When both forms exist, their value and scriptPubKey must agree. Invalid
context exposes no resolved value/script. `valid` means supplied metadata is
internally consistent; it does not authenticate chain membership or unspentness.
Witness-only context cannot be authenticated against a previous transaction.

**Absolute fee only.** After all UTXO consistency checks, rust-bitcoin's checked
fee helper subtracts the output sum from the input sum. JSON uses
`fee: {"status": "available", "fee_sats": 1000}` when computable. Otherwise
`fee_sats` is null and status is `missing_utxo_context`, `invalid_utxo_context`,
`negative_fee`, `overflow`, or `other_error`. Invalid context takes precedence
over missing context. Unsigned-transaction output-total overflow remains a
shared typed analysis error. Amounts are not checked for full consensus monetary
validity. A reported fee is conditional on supplied UTXO data. There is no
feerate field: the final script/witness size is not known, and unsigned vsize
is not a definitive fee-rate denominator.

**Structural signing state.** An input is `finalized` if either final scriptSig
or final scriptWitness is present, even if empty. Otherwise ECDSA partial
signatures, a Taproot key signature, or Taproot script signatures make it
`partially_signed`; absence of those fields makes it `unsigned`. Overall, all
unsigned inputs (including zero inputs) yield `unsigned`, signatures with no
finalized inputs yield `partially_signed`, all finalized yield `finalized`, and
finalized plus non-finalized inputs yield `mixed`. Final markers take precedence
over signatures. Signature presence is not signature validity, and finalization
markers do not establish broadcast readiness. Sighash is never guessed.

**PSBT resource limits.** Total decoded input is capped at **16 MiB**, normalized
base64 at **22,369,624 bytes**, and total text at **22,369,626 bytes** (allowing
CRLF at the full ceiling). These are application limits, not Bitcoin consensus
limits. A framing scan caps all maps and all key/value pairs at 100,000 each,
and the unsigned transaction field at 1,000,000 stripped bytes before semantic
parsing. Aggregate serialized output TapTree values are capped at **12,288
bytes**, bounding dependency tree expansion to at most 4,096 leaves before
allocation. This can reject unusually large valid trees. The dependency also
applies a 4,000,000-byte global-map reader limit and per-field decoder bounds.
The total PSBT is not subjected to the block-weight limit: tests accept large
metadata beyond 4 MB. The shared unsigned-transaction analysis retains its
transaction/report limits. Trailing data, malformed maps, duplicate keys,
truncated data and resource-limit violations fail without partial reports.

**Privacy and scope.** PSBTs can reveal transaction graphs, scripts, signatures,
xpubs, fingerprints and derivation paths. Reports include transaction and
resolved prevout scripts as lowercase hex; review reports before sharing.
Default reports expose counts/presence rather than xpub strings, public keys,
paths, signatures or proprietary/unknown contents. Parsing preserves those
fields internally; this is an inspector, not a sanitizer. Core errors and CLI
argument diagnostics omit full input and upstream privacy-bearing errors.
There is no PSBT logging, network inference, address generation, wallet ownership
claim, signature verification, signing, finalization, extraction, broadcasting,
node/wallet connection, or policy/severity engine.

See [synthetic fixture construction](fixtures/README.md), the
[implementation plan](docs/milestone-2-plan.md), and
[verification record](docs/milestone-2-verification.md).

## Development verification

```sh
cargo fmt --check
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo audit
```

After dependencies have been downloaded, `cargo test --workspace --offline`
runs without network access. Tests use public dummy legacy/SegWit fixtures,
independently computed hash and size expectations, the known genesis
transaction, script/RBF boundaries, all one- and two-byte inputs, deterministic
short malformed buffers, truncation/trailing checks, integer overflow and
resource limits, and real CLI process/JSON checks. No live RPC or private keys
are required. See [fixture notes](crates/txsignx-core/tests/fixtures/README.md).

## Roadmap

- [x] Milestone 1 — Raw transaction analysis
- [x] Milestone 2 — PSBT inspection
- [ ] Milestone 3 — Security policy engine
- [ ] Milestone 4 — Descriptor wallet context
- [ ] Milestone 5 — Bitcoin Core integration
- [ ] Milestone 6 — API/web integration and capstone polish

## Workspace boundaries

This repository contains only `crates/txsignx-core` and `crates/txsignx-cli`.
The sibling [web application](https://github.com/j-kon/txsignx-web) and
[documentation](https://github.com/j-kon/txsignx-docs) are independent repositories.
The outer workspace is not a Git repository. Sibling `txsignx-brand/` remains
local only and must not be initialized, committed, or pushed.

Never commit `.env` files, RPC credentials, seed phrases, mnemonics, private keys,
extended private keys, wallet databases, or API tokens.

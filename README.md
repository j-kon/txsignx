# TxSignX

**Inspect. Verify. Sign with Confidence.**

TxSignX is a Rust-based Bitcoin Transaction Explorer and pre-signing security analyzer. It decodes raw transaction hex, fetches transaction IDs via Bitcoin Core, and inspects PSBT v0 / BIP174 packages. It delivers factual transaction reports and deterministic policy evaluation before keys ever touch a transaction.

## 🎥 Demo Video

[▶ Watch the TxSignX Demo on Loom](https://www.loom.com/share/a6853cc366ef4c419085bb0ee5c599b4)

The demo covers TxSignX transaction inspection and policy workflows:
- **Raw transaction decoding**: Consensus-serialized hex parsing without network assumptions.
- **TXID lookup using Bitcoin Core**: Node-contextualized lookup for confirmed and mempool transactions.
- **Transaction field inspection**: Version, locktime, input sequences, and output values.
- **Script and witness analysis**: Disassembly across standard script types and witness stack inspection.
- **SegWit and explicit RBF detection**: Witness detection and explicit sequence opt-in RBF signaling (`nSequence < 0xFFFFFFFE`).
- **Size, weight, and vsize calculations**: Factual serialization dimensions and consensus weight metrics.
- **Fee and fee-rate calculation from resolved prevouts**: Input/output summation, absolute fee, and sat/vB rate when prevouts are resolved.
- **Confirmed and mempool transaction analysis**: Block hash and confirmation depth for confirmed transactions, mempool state for unconfirmed transactions.
- **PSBT v0 preflight**: Pre-signing security inspection for BIP174 packages.
- **Deterministic PASS, REVIEW, and BLOCK policy decisions**: Rule-driven policy evaluations with scoped results (PASS is never a universal safety guarantee).

## MVP Features

TxSignX inspects transactions across three primary input forms:
- **Raw transaction hex**: consensus-serialized Bitcoin transaction bytes
- **Transaction ID (TXID)**: fetched and contextualized via Bitcoin Core
- **PSBT v0**: unsigned or partially signed BIP174 packages

Factual report capabilities:
- Transaction identification: txid and wtxid
- Version, locktime, and serialized size, weight, and vsize
- Inputs: previous output references, sequences, scriptSig hex, and witness items
- Outputs: satoshi values, scriptPubKey hex, script type, and disassembly
- SegWit presence and explicit RBF signaling (`nSequence < 0xFFFFFFFE`)
- Script classifications: P2PKH, P2SH, P2WPKH, P2WSH, P2TR, OP_RETURN, Unknown

### Raw Transaction Context

Raw transactions do not encode network metadata. Address derivation requires an explicit network (`bitcoin`, `testnet`, `testnet4`, `signet`, `regtest`); mainnet is never silently assumed. Without an explicit network, scriptPubKeys are shown with script type and disassembly only.

### Bitcoin Core Context

When TXID mode is used with a configured node, TxSignX can report:
- Resolved previous outputs (prevouts)
- Total input value, total output value, fee, and fee rate
- Confirmed / mempool status and confirmation count
- Block hash for confirmed transactions (unconfirmed transactions show mempool state without fabricated hashes)
- Standard addresses derived using the node network

Raw transactions do not record previous output values; fees and fee rates remain explicitly unavailable unless prevouts are resolved.

### Pre-Signing Security

For PSBT v0 packages, a deterministic Rust policy engine evaluates transaction risks prior to signing:
- **PASS**: No evaluated active rule requires REVIEW or BLOCK within the evaluated scope. PASS is scoped and does not constitute a universal safety guarantee.
- **REVIEW**: Non-critical warnings detected (e.g., sequence-based RBF replacement signaling).
- **BLOCK**: Critical policy violations detected (e.g., fee ratio exceeding configured safety limits).

Transaction Explorer reports are strictly factual and do not display PASS, REVIEW, or BLOCK verdicts.

## Architecture

```text
Raw Transaction / TXID / PSBT
            |
            v
     Transaction Decoder
            |
     +------+------+
     |             |
Transaction Facts  Bitcoin Core
     |             | Context
     +------+------+
            |
            v
      TxSignX Report
            |
       +----+----+
       |         |
   Explorer   Policy Engine
                 |
          PASS / REVIEW / BLOCK
```

- **Rust source of truth**: Parsing, fact extraction, and policy evaluation logic reside entirely in Rust.
- **Presentation-only frontend**: `txsignx-web` is strictly a presentation layer; the browser does not implement Bitcoin security logic.
- **Deterministic decisions**: Pure rule evaluation only; AI does not decide signing or security outcomes.
- **Isolated credentials**: Bitcoin Core RPC credentials stay server-side; the browser never receives node credentials.

## Run TxSignX

### CLI

Build workspace:
```bash
cargo build --workspace
```

Raw transaction inspection:
```bash
cargo run -p txsignx-cli -- tx inspect <RAW_TX_HEX>
```

TXID inspection via Bitcoin Core:
```bash
cargo run -p txsignx-cli -- tx inspect \
  --txid <TXID> \
  --node-url http://127.0.0.1:8332 \
  --cookie-file ~/.bitcoin/.cookie \
  --network regtest
```

PSBT preflight policy analysis:
```bash
cargo run -p txsignx-cli -- psbt preflight <BASE64_PSBT>
```

### API

Start the local API daemon:
```bash
cargo run -p txsignx-api
```

Endpoints:
- `GET /api/v1/health`
- `POST /api/v1/transactions/inspect`

Raw transaction request:
```json
{
  "raw_transaction": "<HEX>",
  "network": "bitcoin"
}
```

Transaction ID request:
```json
{
  "txid": "<TXID>"
}
```

### Web

The browser presentation layer is available in the [TxSignX Web repository](https://github.com/j-kon/txsignx-web). `txsignx-web` provides a local user interface for:
- PSBT v0 inspection and policy preflight
- Raw Transaction mode with optional network-aware address rendering
- Transaction ID mode backed by the configured local API
- Chain-context display with block metadata and fee analysis

## Safety & Limitations

- **No key handling**: TxSignX does not generate keys, handle seed phrases, hold private keys, or sign transactions.
- **No execution**: There is no transaction finalization or broadcast capability via API or Web.
- **Prevout requirement**: Raw transactions alone cannot determine total input value, fee, or fee rate without resolved prevout context.
- **Explicit networks**: Raw transactions do not encode network metadata; network selection must be explicit.
- **PSBT format**: PSBT support is strictly PSBT v0 / BIP174 (PSBT v2 is not supported).
- **RBF signaling**: Explicit RBF signaling indicates `nSequence < 0xFFFFFFFE`; it does not prove full replaceability.
- **Scoped PASS**: A PASS verdict means no evaluated active rule triggered REVIEW or BLOCK within scope, not that the transaction is universally safe.
- **Credential isolation**: The browser never receives Bitcoin Core RPC URLs, cookie paths, usernames, or passwords.

## Technology

Built with Rust, rust-bitcoin, Bitcoin Core RPC, Axum, React, and TypeScript.

## Project Structure

```text
txsignx/
├── crates/
│   ├── txsignx-core
│   ├── txsignx-policy
│   ├── txsignx-cli
│   └── txsignx-api
└── scripts/
```

- Web Frontend: [TxSignX Web (`txsignx-web`)](https://github.com/j-kon/txsignx-web)
- Documentation: `txsignx-docs`

## Capstone

**Category:** Transaction Explorer

**Project:** TxSignX — A Bitcoin Transaction Explorer and Pre-Signing Security Analyzer

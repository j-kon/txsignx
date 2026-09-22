# Milestone 6 verification

Verified on 2026-09-22 in the Rust repository
`~/Developer/jaykon/txsignx/txsignx`, branch `milestone-6-api`.
Base: `d7d334aa96ae15de3871ad87c4ed3faa7dfdd26e`.

## Automated verification

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace --all-targets --all-features` | PASS |
| `cargo test --workspace` | 282 passed; all 266 baseline tests retained |
| New M6 tests | 16: endpoints 5, preflight 4, security 5, startup 2 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo audit` | 0 known vulnerabilities, 0 warnings; 111 packages, 1,258 advisories |
| `cargo tree` | PASS; one bitcoin 0.32.102 type universe |
| `cargo tree --duplicates` | Only base64 0.13.1 / 0.21.7, used by RPC / bitcoin respectively |
| Web verification (`txsignx-web`) | Oxlint 0 warnings / 0 errors; Vitest 25 tests passed; TypeScript / Vite build PASS |
| Python API integration harness (`scripts/verify-api.py`) | PASS; loopback only, PASS/REVIEW/BLOCK, CORS, media types, size bounds |
| Isolated Regtest harness (`scripts/verify-api-regtest.py`) | PASS; Bitcoin Core v31.1.0, datadir cleanup and process shutdown verified |
| `git diff --check` | PASS |

Build verification used the installed Command Line Tools through per-command
`DEVELOPER_DIR=/Library/Developer/CommandLineTools`. No global developer setting
or Xcode license was changed. Standard unit and integration tests require no live Bitcoin node.

## Architecture and crate separation

`crates/txsignx-api` was added to the Cargo workspace as a local Axum HTTP presentation layer.
The core architecture remains strictly hierarchical:

```
txsignx-web (React + TypeScript + Vite)
  ↓ HTTP JSON
txsignx-api (Local Axum service)
  ↓
txsignx-core + txsignx-wallet + txsignx-node + txsignx-policy
  ↓
PolicyReport
  ↓
PASS / REVIEW / BLOCK
```

- `txsignx-core`, `txsignx-wallet`, `txsignx-node`, and `txsignx-policy` remain completely free of HTTP and Axum dependencies.
- `txsignx-policy` remains the sole authority for security findings, severity, decisions (`PASS`, `REVIEW`, `BLOCK`), risk ratings, and evaluation coverage (`evaluated`, `partially_evaluated`, `not_evaluated`).
- The browser interface never recomputes or overrides policy decisions.

## API contract and endpoints

All routes are versioned under `/api/v1`:

| Method / Path | Purpose |
| --- | --- |
| `GET /api/v1/health` | Public service metadata (`status`, `service`, `version`). No secrets or filesystem paths. |
| `GET /api/v1/capabilities` | Supported operations, engine bounds, active/deferred rule counts, and node configuration status. |
| `GET /api/v1/policies` | Live policy registry directly from `txsignx_policy::rule_catalog()`. |
| `GET /api/v1/policies/:code` | Single rule detail or sanitized 404. |
| `POST /api/v1/transactions/inspect` | Raw transaction analysis reusing `txsignx-core`. |
| `POST /api/v1/psbt/inspect` | PSBT v0 inspection reusing `txsignx-core`. |
| `POST /api/v1/psbt/preflight` | Bounded policy preflight (inspection only, wallet-aware, or node-aware). |

Preflight decisions `pass`, `review`, and `block` all return HTTP 200 with their respective findings.
HTTP error codes (400, 404, 413, 415, 422, 503, 504) indicate transport or validation issues, never policy outcomes.

## Security controls and bounds

1. **Loopback Binding**: Defaults to `127.0.0.1:8080`. Binding to external interfaces requires `--allow-external` and emits a startup warning.
2. **CORS & Host Validation**: Explicit allowed origins (`http://localhost:5173`, `http://127.0.0.1:5173`). Wildcard `*` is prohibited. Host header validation prevents DNS rebinding attacks on loopback.
3. **Hardened Headers**: Responses include `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and restrictive `Content-Security-Policy`.
4. **Transport Resource Bounds**:
   - Request text limit: 1 MiB (`TEXT_BYTES = 1,048,576`)
   - Request body limit: 2 MiB (`BODY_BYTES = 2,097,152`)
   - Serialized response limit: 8 MiB (`RESPONSE_BYTES = 8,388,608`)
   - Max concurrent admitted requests: 4
   - Blocking worker threads: 2 (`tokio::task::spawn_blocking`)
   - HTTP request deadline: 30 seconds
5. **Sanitized Errors**: Errors use `{ "error": { "code": "...", "message": "..." } }`. Upstream RPC errors, parser internals, and request input bodies are never reflected in error responses.
6. **No Secrets or Private Keys**: The API accepts only public descriptors; private keys (`xprv`, `WIF`, seed phrases) are rejected. RPC cookie paths and contents are never exposed.
7. **No Broadcast Endpoint**: The API exposes no broadcast endpoint. Regtest-only broadcast remains an M5 CLI-only feature.

## Isolated Regtest verification

The integration harness `scripts/verify-api-regtest.py` was executed against local Bitcoin Core v31.1.0:
- Temporary datadir created under `/private/var/folders/.../txsignx-m6-regtest-...` (verified distinct from `~/.bitcoin`).
- Dedicated loopback ports, disabled P2P networking (`-connect=0 -dnsseed=0 -listen=0 -networkactive=0 -discover=0`).
- Verified node-aware preflight producing `PASS` with confirmed mature coinbase inputs.
- Verified `TG016` (`NodePrevoutMismatch`) triggering `BLOCK` upon modified prevout context without broadcast.
- Verified HTTP 503 fail-closed behavior when configured node is stopped.
- Clean shutdown and removal of the temporary datadir verified (logical size ~18.3 MB).

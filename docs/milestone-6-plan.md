# Milestone 6 implementation plan

Goal: expose the existing Rust inspection and policy engine through a local Axum
API and an accessible React/TypeScript/Vite product, with reproducible demos.

Specification: the user-provided Milestone 6 brief. Rust base
`d7d334aa96ae15de3871ad87c4ed3faa7dfdd26e`; baseline 266 tests, fmt/check/Clippy
and RustSec passed. Web base `7401d48c76802a48a0d7d25f32a5b8fe88ad8835`;
npm ci/lint/build passed, zero npm advisories.

## Contract and trust boundaries

Browser → React → HTTP JSON → txsignx-api → core/wallet/node/policy. Core,
wallet and policy stay HTTP-free. Policy alone supplies decisions, findings,
severity and evaluation coverage. No frontend TG evaluation or fee inference.

Routes under `/api/v1`:

| Method/path | Request | Successful response |
| --- | --- | --- |
| GET health | none | status, service, version |
| GET capabilities | none | supported features, limits, configured node flag |
| GET policies | none | existing Rust RuleCatalog |
| GET policies/{code} | exact code | existing active/deferred metadata |
| POST transactions/inspect | `{raw_transaction: string}` | TransactionReport |
| POST psbt/inspect | `{psbt: string}` | PsbtReport |
| POST psbt/preflight | typed request below | PreflightReport |

Preflight accepts `psbt`, optional `policy` with optional
`max_absolute_fee_sats`/`max_fee_ratio_bps`, optional `wallet` with required
`network`, `external_descriptor`, `internal_descriptor`, optional
`derivation_window` (1000 default), `expected_change_outputs` (empty default),
and optional `node: {use_configured_node: boolean}`. Unknown JSON fields fail.
Node requests require wallet context; its network must match server configuration.
No browser RPC URL, credentials, cookie content/path or file path is accepted.
Successful PASS/REVIEW/BLOCK all return HTTP 200 with the existing snake_case enums.

Errors use `{error:{code,message}}` with static sanitized text: 400 invalid JSON,
413 size limit, 415 media type, 422 invalid data/context, 404 absent resource,
405 method, 503 node unavailable/busy, 504 timeout, 500 internal failure.
No request or upstream error text is echoed.

## Server security

Default bind 127.0.0.1:8080; explicit nonloopback binding requires an additional
allow-external flag and emits a warning. No authentication/hosting service is
introduced. Explicit origins default to localhost/127.0.0.1:5173, no wildcard.
Reject unapproved Origin before processing, including preflight. Guard Host for
local binding to mitigate DNS rebinding. Reject query strings on analysis routes.
No request/body/descriptor/auth logging. Cache-Control no-store, nosniff,
no-referrer and restrictive CSP headers. No cookie sessions or credentials CORS.

API input text limit 1 MiB (stricter transport profile than CLI); total body 2 MiB;
JSON output 8 MiB using a bounded serializer. Concurrency: four admitted requests,
two CPU/RPC workers with immediate overload rejection. HTTP deadline 30 seconds.
Blocking Rust/RPC work uses spawn_blocking and holds its worker permit even after
client timeout, preventing abandoned jobs from creating an unlimited worker queue.
Existing engine input/derivation/node limits still apply. No cancellation claim
for already-running blocking work. Slow or malicious clients are not a substitute
for a production reverse proxy; this is a local development service.

Server startup owns optional M5 cookie-authenticated loopback node configuration.
Capabilities describe configuration, not liveness. A requested unavailable node
fails closed. No broadcast route: existing advanced Regtest CLI remains separate.

## Product design and behavior

Use brand midnight #0B0F14, surface #111827, border #243041, Security Blue
#1E4B8F, Bitcoin Orange #F7931A, muted #94A3B8, semantic green/amber/red.
Inter/system sans and JetBrains Mono/monospace fallbacks; no external font calls.
Keep brand source assets local-only: use the name, supplied colors and copy.
Home emphasizes the exact supplied headline and a clearly labeled real synthetic
report preview. Inspector uses a two-column workspace: input/context controls and
results, stacked on mobile. Result facts use tables; findings and coverage have
distinct sections, not a repeated marketing-card grid.

Views: Home, Inspector (PSBT/raw), Policies; explicit Inspect/Preflight operation,
optional wallet fields and configured-node switch; clear sensitive-data notice;
sample/upload/paste; loading/error/retry; report copy/download/clear. Reports show
unavailable values explicitly, PSBT signing/fee/RBF facts, input/output details,
sighash/metadata/UTXO consistency, backend findings and all three coverage groups.
Use backend decision verbatim for semantic colors. PASS is limited to evaluated
rules and never a signing authorization. No signing or broadcast button.

Client `VITE_TXSIGNX_API_URL` is explicit; only that API receives input. Runtime
response validation rejects malformed/unknown decisions; bounded reads and timeout;
integer values outside JS safe range must not silently round. No storage, input
in URLs, console dumps, analytics, HTML injection or automatic clipboard writes.
Hosted demo uses public synthetic offline samples and a configured API; it never
pretends to have node facts when no node exists.

## Execution and tests

Use meaningful incremental commits and test-first behavior changes. Independent
web work can proceed against this contract while API implementation proceeds.
The requested feature branches isolate changes; preserve all old branches.

- [ ] API foundation: `crates/txsignx-api/{Cargo.toml,src/lib.rs,src/main.rs,
  src/error.rs,src/config.rs}`; select Axum 0.8.9 features json/http1/tokio,
  Tokio runtime/sync/time/net, tower utilities for in-process tests. Verify
  dependency tree/RustSec and unchanged Bitcoin versions before dependency commit.
- [ ] Metadata/inspection: `src/routes.rs`, `src/request.rs`, tests/endpoints.rs.
  Assert health has no secrets, registry 15/2, detail/404, valid tx/PSBT reports
  and sanitized invalid inputs; compare fixture TXID/fees to existing facts.
- [ ] Preflight: `src/preflight.rs`, tests/preflight.rs. Exercise PASS/REVIEW/BLOCK,
  wallet classification/privacy, skipped/partial coverage, invalid configuration,
  unavailable node and fake node stable/mismatch cases without any live node.
- [ ] HTTP hardening: `src/security.rs`, tests/security.rs. Exercise malformed
  JSON, duplicate/unknown keys, media type, body/output limits, CORS allow/reject,
  Host checks, method/fallback errors, overload and timeout permit retention.
- [ ] Web client and shell: `src/lib/api`, `src/features`, `src/components`, CSS.
  Use test runner appropriate to Vite; test response rejection and public sample
  flows, then implement input/results/registry/export with React presentation only.
- [ ] Web tests/CI: test Home/input/upload, API errors, PASS/REVIEW/BLOCK/coverage,
  escaped hostile text and explicit exports; npm ci/lint/build/test/audit. CI
  uses read-only permissions and pinned trusted actions. Existing Rust CI already
  covers all workspace members; change only if needed for actual M6 requirements.
- [ ] Integration: start owned loopback API/Vite, verify real browser raw/PSBT,
  all decisions, policies, invalid/oversized input and unavailable API. Add safe
  reproducible harness. Node-aware test uses only isolated temporary Regtest with
  all M5 safety flags, explicit datadir/nondefault ports/cookie, stop/size/remove.
- [ ] Documentation/capstone: Rust API README and final verification; web setup,
  privacy and limits; docs repository architecture/security/API/policies/user flow,
  limitations, walkthrough, presentation outline and 3–5 minute video script.
- [ ] Internal security/spec review; fix in-scope findings with regressions. Run
  full 266+ Rust suite, fmt/check/Clippy/audit/tree/diff and web suite. Record actual
  results; normal-push clean branches only after every required check passes.

Excluded: signing, finalization, private keys/seeds, API broadcast, public-chain
node synchronization, persistence/full wallet sync, PSBT v2, mobile/hardware
wallets, remote RPC, AI decisions or post-M6 features. No PRs and no merges.

# Milestone 5: Bitcoin Core chain context

Base: `e1101693e9a98c773f2c1b8c75bb8e1d4772ca7b`. Baseline verified: 224 tests, formatting/check/Clippy clean, RustSec clear, 71 lockfile packages, 10 active / 4 deferred rules.

## Trust and architecture

`txsignx-node` depends on core and the reviewed Bitcoin Core RPC client, never policy or wallet. It builds immutable, serializable node facts bound to the entire inspected transaction and resolved prevout values/scripts/status. Policy consumes optional node and wallet reports without I/O. Core and wallet remain free of RPC. Bitcoin Core is the explicitly selected chain-state authority; these are node-reported observations, not independent consensus proofs.

## Configuration and bounded observations

Cookie authentication only. Accept exactly `http://127.0.0.1:PORT` or `http://[::1]:PORT` with canonical decimal nonzero port. Reject all other hosts, schemes, paths, credentials, queries and fragments before reading authentication. Read a regular cookie file with a strict byte limit; retain authentication only inside the RPC client, never errors, reports or Debug. Review transport redirects, timeouts and response bounds before accepting the dependency.

Require one explicit configured network whenever RPC is requested, independently of wallet configuration. Map Core main/test/testnet4/signet/regtest to canonical networks. Reject unsupported chains, IBD, malformed readiness and headers ahead of blocks. Build against a stable tip, retry the whole build at most three times; validate gettxout bestblock against the snapshot. Query chain-only and mempool-aware gettxout for each input in deterministic order. Additional node input ceiling limits RPC work; no scanning or synchronization.

Both views found means confirmed unspent; only mempool-aware found means unconfirmed; only chain found means spent in mempool; neither means unavailable in the queried views, without historical claims. Inconsistent values/scripts or impossible confirmations are errors. Mempool observations are not an atomic snapshot. Compare integer satoshis and scripts against existing resolved PSBT facts; never repair missing/invalid metadata.

## Policy

Activate TG001 (configured/node network mismatch, critical), TG006 (coinbase confirmations below 100, critical), TG015 (unavailable, critical), TG016 (prevout value/script mismatch, critical), TG017 (mempool spend conflict, high). Registry becomes 15 active / 2 deferred; TG007/TG008 remain deferred. Track evaluated/partial/skipped checks honestly, preserving decisions without node context.

## Broadcast

CLI orchestration requires configured and observed Regtest, full wallet/node context, explicit expected change, all required context checks fully evaluated, and PASS. REVIEW/BLOCK return their normal decision codes without acceptance or send calls. Require final fields on every input, perform checked rust-bitcoin extraction, then one matching allowed testmempoolaccept result before sendrawtransaction. No signing, finalization, force override or non-Regtest broadcast. Runtime/configuration/acceptance errors return 1 without credential/raw RPC echo.

## Verification and delivery

Implement with deterministic fake RPC tests first: configuration/privacy, readiness/tip races, all four availability states, prevout comparison, binding, rule boundaries/statuses, CLI sources, and broadcast call counters. Preserve the 224 existing tests except explicit registry expectations. Review dependency metadata/tree/audit before committing dependency changes; maintain bitcoin 0.32.102 and review all new transitives.

A checked-in harness starts only its own Bitcoin Core 31.1 Regtest with a fresh temporary datadir, dedicated non-default loopback ports, `-connect=0 -dnsseed=0 -listen=0`, no public-chain synchronization. Print safety confirmations before startup. The harness creates/signs/finalizes fixtures using its temporary Core wallet outside production TxSignX; exercise all requested node rules and broadcast allow/reject gates. Always stop only that process, verify exit, report datadir size, delete only its exact task-created directory, and verify removal. Never use user Bitcoin data or modify system configuration.

Run full automated checks, RustSec, prior fixture regressions, real Regtest scenarios and secret/scope review before verification documentation and normal push. Use meaningful incremental commits, preserve all milestone branches, no PR #5. M6, persistence, wallet sync, remote RPC and public-chain broadcast remain out of scope.

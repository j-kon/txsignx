# Milestone 5 verification

Verified on 2026-09-21 in the Rust repository
`~/Developer/jaykon/txsignx/txsignx`, branch `milestone-5-node-context`.
Base: `e1101693e9a98c773f2c1b8c75bb8e1d4772ca7b`.
The final implementation and harness were verified at `cf00db7`; the subsequent
verification commit changes documentation only.

## Automated verification

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace --all-targets --all-features` | PASS |
| `cargo test --workspace` | 266 passed; all 224 baseline tests retained |
| New M5 tests | 42: node 21, policy 10, node CLI 3, broadcast 8 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo audit` | 0 known vulnerabilities, 0 warnings; 78 packages, 1,251 advisories |
| `cargo tree` | PASS; one bitcoin 0.32.102 type universe |
| `cargo tree --duplicates` | Only base64 0.13.1 / 0.21.7, used by RPC / bitcoin respectively |
| CLI help, version, human and JSON policy lists | PASS; version 0.1.0; 15 active / 2 deferred |
| `git diff --check` | PASS |

Build verification used the installed Command Line Tools through per-command
`DEVELOPER_DIR=/Library/Developer/CommandLineTools`. No global developer setting
or Xcode license was changed. Standard tests require no live Bitcoin node.

## Dependencies and architecture

The lockfile grew from 71 to 78 packages: one local crate and six external
packages. New external packages are bitcoincore-rpc 0.19.0,
bitcoincore-rpc-json 0.19.0, jsonrpc 0.18.0, minreq 2.14.1, base64 0.13.1 and
log 0.4.34. Actual metadata, API, features, transport behavior, dependency tree
and RustSec results were reviewed. No existing resolved package version changed;
bitcoin 0.32.102, bdk_wallet 3.1.0 and miniscript 12.3.7 remain unchanged.

minreq is also a direct dependency with default features disabled. It was already
an RPC transitive dependency and adds no further resolved package. The custom
transport implements bitcoincore-rpc's RpcApi to disable redirects, bound
responses and sanitize errors without raw-response logging. No Bitcoin type
conversion shim, advisory suppression or version change to evade an audit was used.

`txsignx-node` depends on core, not wallet, policy or CLI. Core and wallet remain
RPC-free. The testable NodeRpc interface exposes blockchain info, best hash,
height, gettxout, testmempoolaccept and sendrawtransaction. Policy consumes
immutable facts and performs no RPC; CLI orchestrates operations.

## Configuration and chain facts

Only exact literal-loopback HTTP endpoints are accepted:
`http://127.0.0.1:PORT` or `http://[::1]:PORT`, with canonical nonzero decimal
ports. Hostnames, remote addresses, credentials, paths, query strings, fragments,
redirects and other schemes are rejected. No remote RPC is allowed.

Cookie authentication is mandatory. Regular-file checks, a 4,096-byte limit,
cookie syntax validation and sanitized errors protect credentials. Authentication
is not serialized or exposed through Debug, reports or logs. Transport bounds
include a five-second timeout, 1 MiB response body, 8 KiB headers and 1 KiB status
line. Node context permits at most 256 inputs and three complete attempts.

Node mode requires RPC URL, cookie file and one explicit network. Wallet context
is optional for preflight; combined mode shares the same configured network.
Core main/test/testnet4/signet/regtest map to canonical network names. This never
infers a PSBT network. Unsupported/malformed state, IBD, unequal headers/blocks,
invalid progress or unavailable RPC return typed errors without false PASS.

Each snapshot records network, height/hash and ordered per-input facts. Tip
hash/height are checked after observations, including gettxout bestblock. A tip
change retries the whole build, with an UnstableChainTip error after three attempts.
Private report fields and a binding to the complete inspected PsbtReport prevent
reuse with altered transaction or prevout facts. Policy rejects mismatched context.

| gettxout(false) | gettxout(true) | Availability |
| --- | --- | --- |
| Found | Found | ConfirmedUnspent |
| None | Found | MempoolUnconfirmed |
| Found | None | SpentInMempool |
| None | None | NotAvailable |

Impossible confirmations or contradictory views fail closed. Node prevouts are
compared using integer satoshis and script bytes; missing or invalid PSBT metadata
is never repaired. NotAvailable makes no historical claim. SpentInMempool is a
node-reported conflict observation, not a maliciousness judgment. Mempool queries
are not atomic. Bitcoin Core is the selected chain-state authority; TxSignX does
not independently prove consensus state.

## Policy and CLI

| Rule | Result when triggered |
| --- | --- |
| TG001 configured/node network mismatch | CRITICAL / BLOCK |
| TG006 coinbase below 100 confirmations | CRITICAL / BLOCK |
| TG015 unavailable outpoint | CRITICAL / BLOCK |
| TG016 prevout value and/or script mismatch | CRITICAL / BLOCK |
| TG017 mempool spend conflict | HIGH / REVIEW |

There are 15 active rules: TG001–TG006 and TG009–TG017. TG007 and TG008 remain
deferred. Evaluation statuses distinguish fully evaluated, partially evaluated and
not evaluated checks. Missing node context does not silently mark node rules
evaluated; coinbase/prevout checks report partial coverage where appropriate.

Node-aware preflight supports positional, file and stdin PSBT input and safe human
or JSON output. Exit codes remain PASS 0, runtime/configuration error 1, REVIEW 2
and BLOCK 3. Without RPC options, M4 behavior and omission of node_context remain.

## Broadcast safety

Broadcast requires full wallet and node context, explicit expected change,
configured and observed Regtest, fully evaluated required context checks and a
recomputed policy PASS. Caller-supplied decisions are not trusted. REVIEW/BLOCK
return before acceptance or send calls. The PSBT must match the inspected facts,
every input must have appropriate nonempty final fields, and checked rust-bitcoin
transaction extraction must succeed. TxSignX performs no signing or finalization.

Core must return exactly one matching allowed testmempoolaccept result before
sendrawtransaction. Returned TXIDs are checked. The live node's Regtest network,
readiness and original chain tip are rechecked before both acceptance and send.
Core ultimately validates signatures and scripts. Separate RPCs cannot provide
an atomic guarantee against subsequent node/mempool changes; no such claim is made.

Fake RPC call counters cover refusal for TG001/2/3/4/5/6/9/10/11/13/14/15/16/17,
missing context/change, unfinished PSBTs, rejection and node changes between gates.
No non-Regtest broadcast, force override or hidden production wallet RPC exists.

## Backward regression

All existing tests passed. Manual JSON checks confirmed M1 legacy/SegWit reports
still have null fees and no inferred network, and M2 unsigned/partial PSBT signing
states remain correct.

M3 fixtures retained PASS/0; 800k fee TG002+TG003 BLOCK/3; missing UTXO TG010
REVIEW/2; invalid UTXO TG009 BLOCK/3; unusual sighash TG011 REVIEW/2; OP_RETURN
TG014 BLOCK/3; unknown script TG013 REVIEW/2; extension metadata TG012 INFO PASS/0.

M4 retained payment PASS/0, foreign and collaborative inputs TG004 REVIEW/2,
external-keychain change TG005 REVIEW/2, hijack TG005 BLOCK/3 and no duplicate
TG004 for missing/invalid UTXO. Derivation boundaries remained index 999/window
1000 PASS, index 1000/window 1000 REVIEW, index 1000/window 1001 PASS.

## Real Bitcoin Core verification

`scripts/verify-regtest.py` passed against Bitcoin Core 31.1.0 using
`/opt/homebrew/bin/bitcoind` and `/opt/homebrew/bin/bitcoin-cli`. The harness uses
only Python standard-library facilities and a separate temporary Core wallet to
construct, sign and finalize synthetic PSBTs outside production TxSignX.

Final run datadir:
`/private/var/folders/h5/n3ddt_k116j56y80dyxc7hn40000gn/T/txsignx-m5-regtest-c9iszsax`.
RPC port 57402; P2P port 57403 with listening disabled. Startup explicitly used
`-regtest`, this dedicated datadir, loopback RPC, cookie authentication and
`-connect=0 -dnsseed=0 -listen=0 -networkactive=0 -discover=0`.
Safety confirmations were printed before startup. Inactive networking and an
empty peer list were asserted. No public-chain synchronization or user datadir was used.

| Real scenario | Result |
| --- | --- |
| Wallet + node normal preflight | PASS / 0 |
| Positional/file/stdin, human/JSON, privacy | PASS |
| Wrong configured network | TG001 BLOCK / 3 |
| Unavailable UTXO | TG015 BLOCK / 3 |
| Prevout value mismatch | TG016 BLOCK / 3 |
| Prevout script mismatch | TG016 BLOCK / 3 |
| Immature coinbase | TG006 BLOCK / 3 |
| Actual maturity boundaries | 99 BLOCK; 100 and 101 PASS |
| Mempool conflict | TG017 REVIEW / 2 |
| BLOCK broadcast candidate | No broadcast; candidate absent from mempool |
| REVIEW broadcast candidates | No broadcast |
| Non-Regtest configured network | Refused; no broadcast |
| Unfinished PSBT | Error / 1; no broadcast |
| Invalid final witness rejected by Core | Error / 1; no sendrawtransaction |
| Externally finalized PASS candidate | Core allowed; broadcast / 0 |
| RPC unavailable | Error / 1; no false PASS |

Successful broadcast TXID:
`6ed1751fdb18e0cf83e2854b71da920b98a2de65b0115672407f376ea2e9d190`.
Its presence in the temporary Core mempool was verified. IBD/not-ready and
continually changing tip behavior were verified with deterministic fake RPC tests.

Cleanup succeeded: only the harness-owned node was stopped gracefully, its process
exit was verified, and the datadir logical size was reported as 18,417,901 bytes.
Only that exact task-created directory was removed; absence was verified. No user
Bitcoin data, global configuration, services or other node process was changed.

## Internal security and scope review

Reviewed production paths for panic/unwrap/expect/unsafe, unbounded work, URL and
redirect bypass, credential/descriptor leakage, malformed RPC, integer arithmetic,
context binding, chain races and broadcast ordering. New production input paths
contain no panic/unwrap/expect. Response failures use sanitized typed errors.
Regression tests cover a discovered broadcast-time node-switch gap: network,
readiness and bound tip are now rechecked before both side-effect gates.

Human/JSON real-node output was checked against cookie and public descriptor/xpub
contents without printing secrets. Tracked changes contain no private fixtures,
cookie files, wallet databases, Bitcoin datadir contents, credentials or private
keys. Authorization handling is implementation code, not an embedded credential.
External signing/finalization RPCs occur only in the explicit temporary Regtest
harness. This is an internal review, **not a professional security audit**.

Only the Rust repository is changed. No persistence, full wallet sync, balance or
history service, remote blockchain API, public-chain broadcast, application server
or Milestone 6 work was added. No reset, clean, rebase, squash, force push or history
rewriting was used. Milestone 1–4 branches remain preserved, and sibling web/docs/
brand repositories are untouched. Delivery is a normal branch push only; no PR #5.

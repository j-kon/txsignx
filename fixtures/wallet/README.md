# Public descriptor wallet fixtures

Synthetic public-key-only fixtures; never fund these scripts. The generator uses
a known public curve point and dummy chain code, without deriving a seed or
private key. External and internal descriptors use `/0/*` and `/1/*` respectively;
`/2/*` supplies non-matching scripts. These descriptors are public demo data,
not a real wallet. Regenerate to stdout with:

```sh
cargo run -p txsignx-wallet --example wallet_fixtures
```

The output is TSV: filename and text. Production classification does not write
files. Use `external.desc` and `internal.desc`, network regtest, default window
1000, and `--expected-change-output 1` with these PSBTs:

| File | Expected result |
|---|---|
| payment.b64 | External input 7; internal change 3; PASS/0 |
| change-hijack.b64 | Expected change unmatched; TG005 Critical BLOCK/3 |
| external-change.b64 | Expected change external 3; TG005 High REVIEW/2 |
| foreign-input.b64 | Valid unmatched prevout; TG004 High REVIEW/2 |
| collaborative.b64 | One matched, one unmatched input; TG004 REVIEW/2 |
| missing-utxo.b64 | Unavailable missing prevout; TG010 REVIEW/2, no TG004 |
| invalid-utxo.b64 | Unavailable inconsistent prevout; TG009 BLOCK/3, no TG004 |
| boundary-999.b64 | External input 999; PASS/0 with window 1000 |
| boundary-1000.b64 | Unmatched at window 1000 (TG004 REVIEW/2), matched at 1001 |
| index-zero.b64 | External input 0, internal change 0; PASS even at window 1 |

No expected-change flag means TG005 is not evaluated. A no-match is bounded and
cannot prove universal non-ownership. The fixtures supply internally consistent
prevout facts except where explicitly testing missing/invalid context; no chain
existence, balance, cryptographic signing validity or network truth is asserted.

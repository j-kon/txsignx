# Transaction fixtures

`legacy.hex` and `segwit.hex` contain deterministic public dummy transactions.
`tests/common/mod.rs::fixture_transaction` constructs their exact equivalents
using rust-bitcoin; a test checks serialized byte equality. These are structurally
decodable test transactions, not valid signed spends or real wallet data.

Both use version 2, locktime 42, one input referencing a repeated `11` TXID at
vout 1, scriptSig `0151`, and sequence `0xfffffffd`. Outputs are 100,000 sats to
a P2PKH script containing a repeated `22` hash and 50,000 sats to a P2WPKH script
containing a repeated `33` hash. The SegWit fixture adds marker/flag `0001` and
witness items `010203`, empty, and `abcd`. No key or real signature is included.

| Fixture | Bytes | Stripped bytes | Weight (WU) | Vsize (vB) |
| --- | ---: | ---: | ---: | ---: |
| Legacy | 118 | 118 | 472 | 118 |
| SegWit | 129 | 118 | 483 | 121 |

Both TXIDs (and the legacy wTXID):
`15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6`

SegWit wTXID:
`561d35cd60944685cbc9155bb5ea54de63aa4ec39c4ac3f2aa936f127cbeccd1`

These expected values were calculated independently using Python hashlib's
double SHA-256 over the fixed bytes and reversed for Bitcoin display order.
Weight is three times stripped size plus total size; vsize rounds weight/4 up.
The legacy fixture is exactly the stripped serialization of the SegWit fixture.
The tests use literal expectations, not the analyzer's calculations.

To independently reproduce the display hash of either file (the full SegWit
hash is its wTXID; the legacy hash is the shared TXID):

```python
from hashlib import sha256
from pathlib import Path

for name in ("legacy", "segwit"):
    raw = bytes.fromhex(Path(f"crates/txsignx-core/tests/fixtures/{name}.hex").read_text())
    print(name, len(raw), sha256(sha256(raw).digest()).digest()[::-1].hex())
```

The separate genesis test uses rust-bitcoin's known Bitcoin genesis transaction
and verifies TXID `4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b`,
204 bytes, 816 WU, and 5,000,000,000 output satoshis. Its network selection is
solely for fixture acquisition; the production analyzer has no network selection
or inference.

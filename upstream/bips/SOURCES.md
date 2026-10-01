# Sources: Bitcoin Improvement Proposals used by friglet-ui

Retrieved 2026-10-01 from https://github.com/bitcoin/bips at commit
`3a10b5b5f0a7586df8928d580a3009744ebb2079` (`master` on that day). Every file is a
byte-identical copy of the upstream file at that commit, fetched from
`https://raw.githubusercontent.com/bitcoin/bips/<commit>/<path>`. Paths match the upstream
repository.

| File | Upstream path | Last upstream change to the file | License |
|---|---|---|---|
| `bip-0392.mediawiki` | `bip-0392.mediawiki` | `41f99576308d192526093a39718fff842692d13c` | BSD-2-Clause (header of the file) |
| `bip-0352.mediawiki` | `bip-0352.mediawiki` | `a99b3d36a7eb612911cac08ecda66a07285170ff` | BSD-2-Clause (header of the file) |
| `bip-0352/send_and_receive_test_vectors.json` | `bip-0352/send_and_receive_test_vectors.json` | `c2ac36f48f71615984087fd151f410457edfed72` | BSD-2-Clause (part of BIP-352) |
| `bip-0380.mediawiki` | `bip-0380.mediawiki` | `93607da01df07348dcd37ddf62d3cae350436783` | BSD-2-Clause (header of the file) |

## Why each file is here

- **BIP-392** (`sp()` descriptors) defines the `spscan`/`tspscan` key encoding friglet-ui
  parses. At this commit its "Test Vectors" section reads `TBD`, so it has no vectors of its
  own. Its example descriptors are used as parser tests.
- **BIP-352** defines the Silent Payments address (`sp`/`tsp`, version 0,
  `B_scan || B_spend`). The receiving test vectors (scan and spend keys with their expected
  addresses) are the address-derivation vectors for friglet-ui's descriptor tests.
- **BIP-380** defines the descriptor checksum (`#` plus 8 characters) and the key-expression
  grammar. Its checksum and key-expression test vectors are used as parser tests.

## Use

Reference material only. `friglet-ui`'s unit tests read these files from disk at test time
(`friglet-ui/src/descriptor.rs`); nothing is compiled into the binary. To refresh, re-fetch
the four paths at a new commit, update this table, and rerun `cargo test -p friglet-ui`.

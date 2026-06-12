# 5G NR LDPC base-graph reference tables — provenance

External reference data used to validate the compiled-in 3GPP TS 38.212
base-graph shift tables in `crates/gf2-coding/src/ldpc/nr_5g/{bg1.rs,bg2.rs}`
(consumed by `crates/gf2-coding/tests/nr5g_external_vectors.rs`).

## Files

| File | Contents | Edges | SHA-256 |
|------|----------|-------|---------|
| `5G_bg1.csv` | TS 38.212 Table 5.3.2-2 (BG1, 46x68), all 8 per-`i_LS` shift columns | 316 | `4ae766634def05dc45e4903c775a5ad7e1feb067747a644504b6d8c90d33b1ed` |
| `5G_bg2.csv` | TS 38.212 Table 5.3.2-3 (BG2, 42x52), all 8 per-`i_LS` shift columns | 197 | `4f4db6f7607446984c0b1dc6243bff4286f6fbad3a477f29d84a5e0edab523fb` |

## Upstream source

- Project: NVIDIA Sionna (an independent, published 5G NR implementation)
- Repository: <https://github.com/NVlabs/sionna>
- Upstream paths:
  - `src/sionna/phy/fec/ldpc/codes/5G_bg1.csv` (blob `ad2472a1b9954ffec152fff834e8edda667660a2`)
  - `src/sionna/phy/fec/ldpc/codes/5G_bg2.csv` (blob `f8dc34c19acd540ba941b12285fa5b4acdc68d24`)
- Fetched: 2026-06-12, from the default branch `main` at repository commit
  `04ddb9312116b408093b9d3ad363a3df355093a6`. The files were last modified
  upstream in commit `9ca7cc75a6431a8d05c4059f3b137ba52a06ce5b` (2025-03-18).
- License: Apache-2.0 (SPDX-License-Identifier in the repository `LICENSE`;
  copyright (c) 2021-2026 NVIDIA CORPORATION & AFFILIATES). Apache-2.0
  permits redistribution with attribution; this file is the attribution.
- Transformation applied: **none** — the files are byte-identical copies of
  the upstream blobs (verify with the SHA-256 sums above).

## Format

Semicolon-delimited CSV with two header lines. Each subsequent line is one
base-graph edge:

```
Row index;Column index;Set index ;;;;;;;
;;0;1;2;3;4;5;6;7
0;0;250;307;73;223;211;294;0;135
;1;69;19;15;16;198;118;0;227
```

- Column 1: base-graph row index. Blank means "same row as the previous
  line" (run-length encoding of the row index).
- Column 2: base-graph column index.
- Columns 3..10: the shift value `V` for lifting set `i_LS` = 0..7
  (TS 38.212 Table 5.3.2-1 defines the sets). Entries absent from the file
  are `-1` (no connection / zero circulant block).

The actual circulant shift applied for a lifting size `Z` is `V mod Z`.

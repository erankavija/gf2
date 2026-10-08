# 5G NR LDPC external reference data — provenance

External reference data consumed by
`crates/gf2-coding/tests/nr5g_external_vectors.rs`: the base-graph tables
validate the compiled-in 3GPP TS 38.212 shift tables in
`crates/gf2-coding/src/ldpc/nr_5g/{bg1.rs,bg2.rs}`, and the
[encoder vectors](#encoder-message-and-codeword-vectors) validate rate-matched
encoding and decoding.

## Files

| File | Contents | Edges | SHA-256 |
|------|----------|-------|---------|
| `5G_bg1.csv` | TS 38.212 Table 5.3.2-2 (BG1, 46x68), all 8 per-`i_LS` shift columns | 316 | `4ae766634def05dc45e4903c775a5ad7e1feb067747a644504b6d8c90d33b1ed` |
| `5G_bg2.csv` | TS 38.212 Table 5.3.2-3 (BG2, 42x52), all 8 per-`i_LS` shift columns | 197 | `4f4db6f7607446984c0b1dc6243bff4286f6fbad3a477f29d84a5e0edab523fb` |

## Upstream source

- Project: NVIDIA Sionna (`@/citation/Sionna2026`; an independent, published
  5G NR implementation)
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

## Encoder message and codeword vectors

| File | Contents | SHA-256 |
|------|----------|---------|
| `sionna_ldpc5g_vectors.txt` | messages and the rate-matched codewords of Sionna's `LDPC5GEncoder`, per rate-matched configuration | `d4ffb6851cee743085ecb9f51d785f6785df9a7ecbb975b48c54d888428b76b1` |
| `generate_sionna_vectors.py` | the generator; gf2 code under the repository licence | — |

### Upstream source

- Project: NVIDIA Sionna (`@/citation/Sionna2026`), class
  `sionna.phy.fec.ldpc.encoding.LDPC5GEncoder`.
- Repository: <https://github.com/NVlabs/sionna>, tag `v2.0.1`, commit
  `04ddb9312116b408093b9d3ad363a3df355093a6`.
- Distribution executed: PyPI wheel `sionna_no_rt-2.0.1-py3-none-any.whl`
  (SHA-256 `62e919c8db9e6c046ba2abb96345bbb3ff0385318c4a54d5b1a413a6b8da8b01`),
  the same release without the ray-tracing dependency.
- Generated: 2026-10-08.
- License: Apache-2.0 (the repository `LICENSE` at `v2.0.1`, SHA-256
  `78d4571703b4ae47d96f32d678896a76749fbf0799db83c177f40be1c721375e`, also
  shipped in the wheel; copyright (c) 2021-2026 NVIDIA CORPORATION &
  AFFILIATES). The repository has no `NOTICE` file at that tag. Apache-2.0
  section 4 conditions redistribution of the Work and of Derivative Works on
  a copy of the licence, retained notices and a statement of changes. The
  vector file contains no Sionna source or data file: it is the output of
  executing the unmodified encoder on messages chosen here. This section is
  the attribution.
- Transformation applied: the encoder output bits are written as hexadecimal
  text; nothing else.

### Generation

`generate_sionna_vectors.py` reproduces the file byte for byte, and with
`--check` compares instead of writing. Environment: CPython 3.12.12 with

```
sionna-no-rt==2.0.1
torch==2.9.1+cpu        # https://download.pytorch.org/whl/cpu
numpy==2.5.3
scipy==1.18.1
h5py==3.16.0
matplotlib==3.11.2
importlib-resources==7.1.0
```

The messages are the leading bits of a SHA-256 counter stream keyed by the
configuration and frame index (`message_bits` in the generator), so they do
not depend on a random-number library.

### Correspondence to `Nr5gRateMatchedCode`

The encoder is constructed as `LDPC5GEncoder(k, n, num_bits_per_symbol=None,
bg=...)` and called without `rv`. In that mode it appends zero filler bits to
`K_b·Z`, encodes with the mother code, removes the filler columns and the
first `2Z` systematic columns, and returns the first `n` remaining bits in
ascending column order, with no output interleaver and no redundancy-version
offset. That is the transmitted word of
`QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k)`. The base graph is an explicit
argument on both sides; the lifting size is selected by each implementation
and compared by the test.

### Format

ASCII text. Lines starting with `#` are comments; the third names the Sionna
distribution version observed at generation.

```
config bg=<1|2> n=<n> k=<k> z=<Z> k_filler=<K_b·Z − k>
frame <message> <codeword>
```

A `config` line opens a configuration with the lifting size and filler count
Sionna reports; the `frame` lines up to the next `config` line belong to it.
`<message>` holds `k` bits and `<codeword>` `n` bits as lowercase hexadecimal:
digit `j` holds bits `4j..4j+3` with the lowest index in the most significant
position, and the final digit is zero-padded. Each configuration holds enough
frames for one full wave of the batch decoder and an incomplete final wave.

## §5.4.2.2 bit-interleaver external validation

The TS 38.212 clause 5.4.2.2 rate-matching bit interleaver implemented in
`crates/gf2-coding/src/ldpc/nr_5g/interleaver.rs` (`output_interleaver`) was
validated against:

1. The spec text of clause 5.4.2.2 (TS 38.212 V16.4.0, 2020-12), which defines
   the mapping verbatim as

   ```text
   for j = 0 to E/Qm - 1
       for i = 0 to Qm - 1
           f_{i + j*Qm} = e_{i*(E/Qm) + j}
   ```

2. NVIDIA Sionna's `LDPC5GEncoder.generate_out_int`
   (`src/sionna/phy/fec/ldpc/encoding.py`, default branch `main` as of
   2026-06-12; Apache-2.0, the same project as the BG1/BG2 tables above), which
   builds the identical gather permutation
   `perm_seq[i + j*num_bits_per_symbol] = i*(n/num_bits_per_symbol) + j` and the
   inverse via `np.argsort(perm_seq)`.

   <https://github.com/NVlabs/sionna/blob/main/src/sionna/phy/fec/ldpc/encoding.py>

The worked-example unit tests in `interleaver.rs`
(`test_worked_example_qm2_e6` → `perm = [0,3,1,4,2,5]`, plus `Q_m ∈ {4,6}`
cases) are derived directly from the spec loop and reproduce
`generate_out_int(E, Q_m)` for those parameters.

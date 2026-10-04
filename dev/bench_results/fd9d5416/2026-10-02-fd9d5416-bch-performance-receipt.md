# Receipt: BCH performance after the cutover (jit:fd9d5416)

Rendered by `dev/active/fd9d5416/render_receipt.py` under `dev/active/fd9d5416/receipt-protocol.md`; every figure is computed at render time from the files listed under *Pinned inputs*.

| Field | Value |
|---|---|
| Issue | `fd9d5416` |
| gf2 revision | `eb557549529294f3090b6f7dd63ff8f0e11962c9` |
| Tree state | clean (git status --porcelain was empty) |
| Host | AMD Ryzen 9 5900X 12-Core Processor |
| CPU flags (kernel bundle) | avx2 bmi2 pclmulqdq vpclmulqdq |
| nproc | 24 |
| Governor (cpu0) | powersave |
| Kernel | Linux 7.2.6-arch2-1 x86_64 |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |
| Cargo | cargo 1.97.0 (c980f4866 2026-06-30) |
| Measurement window (UTC) | 2026-10-02T01:36:34Z to 2026-10-02T03:18:46Z |
| Load average at run start | 04:36:34 up 12 days,  6:41,  6 users,  load average: 2.27, 1.35, 1.12 |
| Message seed (W1/W2) | `0xAE03_BCD0` (`BCH_CORPUS_SEED` at `eb557549529294f3090b6f7dd63ff8f0e11962c9`) |
| Bootstrap | 10000 resamples, one-sided 95%, seed `0xae03bcd0` per cell ID, acceptance >= 0.98 |

## Invocations

| Bench target | Invocation | Started (UTC) | Load avg at start | Finished (UTC) | Load avg at end |
|---|---|---|---|---|---|
| `batch_operations` | `env CARGO_CI_NO_LOCK=1 GF2_BCH_DISPATCH_RECORD=/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-d1b4f85e-run/dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/dispatch.jsonl ./dev/scripts/ccx1-bench-flock.sh --full-host ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench batch_operations -- bch_ --noplot` | 2026-10-02T01:36:34Z |  04:36:34 up 12 days,  6:41,  6 users,  load average: 2.27, 1.35, 1.12 | 2026-10-02T01:37:31Z |  04:37:31 up 12 days,  6:42,  6 users,  load average: 1.46, 1.29, 1.11 |
| `bch_parallel` | `env CARGO_CI_NO_LOCK=1 GF2_BCH_DISPATCH_RECORD=/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-d1b4f85e-run/dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/dispatch.jsonl ./dev/scripts/ccx1-bench-flock.sh --full-host ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench bch_parallel -- bch_ --noplot` | 2026-10-02T01:37:31Z |  04:37:31 up 12 days,  6:42,  6 users,  load average: 1.46, 1.29, 1.11 | 2026-10-02T01:38:38Z |  04:38:38 up 12 days,  6:43,  6 users,  load average: 1.22, 1.24, 1.10 |
| `bch_encode_w1` | `env CARGO_CI_NO_LOCK=1 GF2_BCH_DISPATCH_RECORD=/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-d1b4f85e-run/dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/dispatch.jsonl RAYON_NUM_THREADS=6 ./dev/scripts/ccx1-bench-flock.sh ./scripts/cargo-budget.sh cargo bench -p gf2-coding --features parallel --bench bch_encode_w1 -- --noplot` | 2026-10-02T01:38:38Z |  04:38:38 up 12 days,  6:43,  6 users,  load average: 1.22, 1.24, 1.10 | 2026-10-02T02:32:06Z |  05:32:06 up 12 days,  7:37,  6 users,  load average: 5.91, 5.24, 3.72 |
| `bch_genmatrix` | `env CARGO_CI_NO_LOCK=1 GF2_BCH_DISPATCH_RECORD=/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-d1b4f85e-run/dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/dispatch.jsonl GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench bch_genmatrix -- --noplot` | 2026-10-02T02:32:06Z |  05:32:06 up 12 days,  7:37,  6 users,  load average: 5.91, 5.24, 3.72 | 2026-10-02T03:18:46Z |  06:18:46 up 12 days,  8:23,  6 users,  load average: 1.00, 1.00, 1.13 |

## REQ-01 non-regression against the pinned pre-cutover receipt (`88ca7d2f`)

Cell mapping: `dev/active/591a1c5e/smoke-run.md`. Ratio is throughput new/baseline = median(baseline)/median(new) over per-iteration times; *Lower bound* is the one-sided lower 95% bootstrap bound.

| Cell (same ID both sides) | New median | Baseline median | Ratio | Lower bound | Verdict |
|---|---|---|---|---|---|
| `bch_batch_decode/1` | 24.65 µs | 34.4 µs | 1.3953 | 1.3948 | pass |
| `bch_batch_decode/10` | 11.3 ms | 22.65 ms | 2.0038 | 2.0033 | pass |
| `bch_batch_decode/50` | 61.48 ms | 123.1 ms | 2.0028 | 2.0024 | pass |
| `bch_batch_decode/100` | 124.2 ms | 248.8 ms | 2.0033 | 2.0027 | pass |
| `bch_single_vs_batch/single_loop` | 61.5 ms | 123.2 ms | 2.0033 | 2.0026 | pass |
| `bch_sequential_vs_batch/sequential_loop` | 9.044 µs | 108.1 µs | 11.9542 | 11.9502 | pass |
| `bch_sequential_vs_batch/batch_operation` | 6.45 µs | 107.7 µs | 16.6988 | 16.6764 | pass |

Baseline cells outside the comparison, reported only:

| Baseline ID | New ID | New median | Reason |
|---|---|---|---|
| `bch_batch/1` | `bch_encode_pns_16383_16215/1` | 63.71 µs | renamed; the baseline code (16200, 16008) has no canonical counterpart |
| `bch_batch/10` | `bch_encode_pns_16383_16215/10` | 638.8 µs | renamed; the baseline code (16200, 16008) has no canonical counterpart |
| `bch_batch/50` | `bch_encode_pns_16383_16215/50` | 3.202 ms | renamed; the baseline code (16200, 16008) has no canonical counterpart |
| `bch_batch/100` | `bch_encode_pns_16383_16215/100` | 6.398 ms | renamed; the baseline code (16200, 16008) has no canonical counterpart |
| `bch_single_vs_batch/batch_api` | — | — | removed; the canonical DVB-T2 decoder has no batch call |
| — | `bch_single_vs_batch/decode_into_loop` | 61.57 ms | new; allocation-free decode, no baseline cell |

## REQ-01 non-regression against the survey's pre-cutover gf2 cells (`4e732b56`)

Baseline: the survey's `gf2` rows (legacy `BchEncoder::encode_batch` and `generator_matrix` at the survey revision), per-call time from each trial. New: the `fresh-alloc`, `W1` Criterion cell of the same row and batch.

| New cell | New median | Survey gf2 median | Ratio | Lower bound | Verdict |
|---|---|---|---|---|---|
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=1` | 87.22 ns | 1.077 µs (7 trials) | 12.3488 | 12.3383 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=16` | 936.5 ns | 11.4 µs (7 trials) | 12.1756 | 12.1335 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=256` | 15.47 µs | 216.2 µs (7 trials) | 13.9736 | 13.8199 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=4096` | 332.4 µs | 3.307 ms (7 trials) | 9.9487 | 9.9314 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=1` | 382.8 ns | 35.66 µs (7 trials) | 93.1736 | 92.9466 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=16` | 4.857 µs | 515.4 µs (7 trials) | 106.1201 | 106.0317 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=256` | 178.8 µs | 8.57 ms (7 trials) | 47.9352 | 47.8593 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=4096` | 3.214 ms | 137.3 ms (7 trials) | 42.7154 | 42.4670 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=1` | 817.8 ns | 67.41 µs (7 trials) | 82.4309 | 82.3067 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=16` | 12.81 µs | 999.2 µs (7 trials) | 77.9990 | 77.8326 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=256` | 448.8 µs | 16.45 ms (7 trials) | 36.6505 | 36.5417 | pass |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=4096` | 7.73 ms | 263.9 ms (7 trials) | 34.1390 | 34.0993 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=1` | 160.5 µs | 8.99 ms (7 trials) | 56.0282 | 55.7715 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=16` | 2.972 ms | 146.9 ms (7 trials) | 49.4219 | 48.6105 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=256` | 47.66 ms | 2.336 s (7 trials) | 49.0109 | 48.9907 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=4096` | 762.5 ms | 37.52 s (3 trials) | 49.2060 | 49.1704 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=1` | 797 µs | 47.7 ms (7 trials) | 59.8516 | 59.6421 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=16` | 13.03 ms | 757.7 ms (7 trials) | 58.1400 | 58.1112 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=256` | 209.7 ms | 12.11 s (7 trials) | 57.7450 | 57.6278 | pass |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=4096` | 3.362 s | projection only |  |  | excluded (no measured baseline) |
| `bch_genmatrix_w2/materialize/fresh-alloc/B1` | 45.47 ns | 11.72 µs (7 trials) | 257.6716 | 257.4657 | reported; excluded: legacy timed region includes code construction (`cs.build().generator_matrix()`); the Criterion cell times `generator_matrix()` on a prebuilt code |
| `bch_genmatrix_w2/materialize/fresh-alloc/B2` | 488.3 ns | 1.158 ms (7 trials) | 2370.6801 | 2353.5460 | reported; excluded: legacy timed region includes code construction (`cs.build().generator_matrix()`); the Criterion cell times `generator_matrix()` on a prebuilt code |
| `bch_genmatrix_w2/materialize/fresh-alloc/B3` | 1.526 µs | 7.719 ms (7 trials) | 5058.6270 | 5019.7789 | reported; excluded: legacy timed region includes code construction (`cs.build().generator_matrix()`); the Criterion cell times `generator_matrix()` on a prebuilt code |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2S` | 1.811 s | 31.68 s (3 trials) | 17.4891 | 17.4726 | reported; excluded: legacy timed region includes code construction (`cs.build().generator_matrix()`); the Criterion cell times `generator_matrix()` on a prebuilt code |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2N` | 44.51 s | projection only |  |  | excluded (no measured baseline) |

## Determinism across worker counts and paths

One group per W1 row and batch and per W2 group and row, from `dispatch.jsonl`: every path of a group must record one output digest.

| Group | Row | B | Paths | Workers | Pool width | Kernels | Digest | Verdict |
|---|---|---|---|---|---|---|---|---|
| `bch_encode_w1` | B1 | 1 | 8 | 1, 6 | 6 | avx2-pclmul, scalar | `679f9f02a83b5e3b` | agree |
| `bch_encode_w1` | B1 | 16 | 8 | 1, 6 | 6 | avx2-pclmul, scalar | `86b09af2a95f9902` | agree |
| `bch_encode_w1` | B1 | 256 | 8 | 1, 6 | 6 | avx2-pclmul, scalar | `96097ddf68b6a4a9` | agree |
| `bch_encode_w1` | B1 | 4096 | 8 | 1, 6 | 6 | avx2-pclmul, scalar | `2ecb2f04450ea7db` | agree |
| `bch_encode_w1` | B2 | 1 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `bbf1fae1b23826a6` | agree |
| `bch_encode_w1` | B2 | 16 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `66d67d33fc890220` | agree |
| `bch_encode_w1` | B2 | 256 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `34d390d2ab2300d3` | agree |
| `bch_encode_w1` | B2 | 4096 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `9f269373eb6827f2` | agree |
| `bch_encode_w1` | B3 | 1 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `d36b3689c4bdf7b5` | agree |
| `bch_encode_w1` | B3 | 16 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `cbcc428eb227c4a1` | agree |
| `bch_encode_w1` | B3 | 256 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `eaab55130944c5ad` | agree |
| `bch_encode_w1` | B3 | 4096 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `af34a38c715507a5` | agree |
| `bch_encode_w1` | N1 | 1 | 4 | 1, 6 | 6 | — | `72bea2dcb1c8caea` | agree |
| `bch_encode_w1` | N1 | 16 | 4 | 1, 6 | 6 | — | `bc3c9c1c3f507ee6` | agree |
| `bch_encode_w1` | N1 | 256 | 4 | 1, 6 | 6 | — | `67bafea2e722d1a5` | agree |
| `bch_encode_w1` | N1 | 4096 | 4 | 1, 6 | 6 | — | `746eab602ede8207` | agree |
| `bch_encode_w1` | N2 | 1 | 4 | 1, 6 | 6 | — | `a6167245511a7a39` | agree |
| `bch_encode_w1` | N2 | 16 | 4 | 1, 6 | 6 | — | `ddf3f8148faca7c7` | agree |
| `bch_encode_w1` | N2 | 256 | 4 | 1, 6 | 6 | — | `7d2cffda9390dba5` | agree |
| `bch_encode_w1` | N2 | 4096 | 4 | 1, 6 | 6 | — | `c87965ba94178780` | agree |
| `bch_encode_w1` | N3 | 1 | 4 | 1, 6 | 6 | — | `69f999a0711cc145` | agree |
| `bch_encode_w1` | N3 | 16 | 4 | 1, 6 | 6 | — | `fbf92c3d4badbccc` | agree |
| `bch_encode_w1` | N3 | 256 | 4 | 1, 6 | 6 | — | `18c144764f36454b` | agree |
| `bch_encode_w1` | N3 | 4096 | 4 | 1, 6 | 6 | — | `b75a9513668bde2c` | agree |
| `bch_encode_w1` | N4 | 1 | 4 | 1, 6 | 6 | — | `199db09e07c02733` | agree |
| `bch_encode_w1` | N4 | 16 | 4 | 1, 6 | 6 | — | `bf8450cefcf00d55` | agree |
| `bch_encode_w1` | N4 | 256 | 4 | 1, 6 | 6 | — | `aa1e793096cf1d23` | agree |
| `bch_encode_w1` | N4 | 4096 | 4 | 1, 6 | 6 | — | `9520c58e8c6eda58` | agree |
| `bch_encode_w1` | T2N | 1 | 2 | 1 | 6 | — | `40a7e085ddaa2837` | agree |
| `bch_encode_w1` | T2N | 16 | 2 | 1 | 6 | — | `06f2c954c436ea99` | agree |
| `bch_encode_w1` | T2N | 256 | 2 | 1 | 6 | — | `9bcd0c3c76ccd4bf` | agree |
| `bch_encode_w1` | T2N | 4096 | 2 | 1 | 6 | — | `f591c77d13091245` | agree |
| `bch_encode_w1` | T2N-mother | 1 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `3e4c16f330d368e3` | agree |
| `bch_encode_w1` | T2N-mother | 16 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `77eba9ad7be3845b` | agree |
| `bch_encode_w1` | T2N-mother | 256 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `50cfdf05d9814d5e` | agree |
| `bch_encode_w1` | T2N-mother | 4096 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `7869c1101bf188d2` | agree |
| `bch_encode_w1` | T2S | 1 | 2 | 1 | 6 | — | `b31df8e6880d6f09` | agree |
| `bch_encode_w1` | T2S | 16 | 2 | 1 | 6 | — | `938836d8494ce87b` | agree |
| `bch_encode_w1` | T2S | 256 | 2 | 1 | 6 | — | `bbd14425c7116826` | agree |
| `bch_encode_w1` | T2S | 4096 | 2 | 1 | 6 | — | `964ee79a1dd33b31` | agree |
| `bch_encode_w1` | T2S-mother | 1 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `099219f0a9f24420` | agree |
| `bch_encode_w1` | T2S-mother | 16 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `288b77abea0aa8df` | agree |
| `bch_encode_w1` | T2S-mother | 256 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `63ac6b811bf6b6f8` | agree |
| `bch_encode_w1` | T2S-mother | 4096 | 9 | 1, 6 | 6 | avx2-pclmul, scalar | `aa02818a36fdf698` | agree |
| `bch_genmatrix_w2` | B1 | — | 4 | 1 | 1 | — | `4921f7beb5626955` | agree |
| `bch_genmatrix_w2` | B2 | — | 4 | 1 | 1 | — | `e767d164c0cd6fbe` | agree |
| `bch_genmatrix_w2` | B3 | — | 4 | 1 | 1 | — | `c4ae2d9295215375` | agree |
| `bch_genmatrix_w2` | N1 | — | 4 | 1 | 1 | — | `3df79eb862b058af` | agree |
| `bch_genmatrix_w2` | N2 | — | 4 | 1 | 1 | — | `a27f356b6bf81d09` | agree |
| `bch_genmatrix_w2` | N3 | — | 4 | 1 | 1 | — | `e2ec54d28ddcf489` | agree |
| `bch_genmatrix_w2` | N4 | — | 4 | 1 | 1 | — | `598e4f9b94fda084` | agree |
| `bch_genmatrix_w2` | T2N | — | 2 | 1 | 1 | — | `ad96503e924ca3dc` | agree |
| `bch_genmatrix_w2` | T2N-mother | — | 4 | 1 | 1 | — | `23590f5e2edc2e11` | agree |
| `bch_genmatrix_w2` | T2S | — | 2 | 1 | 1 | — | `f69a2a68f2a25f96` | agree |
| `bch_genmatrix_w2` | T2S-mother | — | 4 | 1 | 1 | — | `a90dc8be3de0f72d` | agree |
| `bch_paritycheck` | B1 | — | 2 | 1 | 1 | — | `ca502590b08d1079` | agree |
| `bch_paritycheck` | B2 | — | 2 | 1 | 1 | — | `72e8731c508999fa` | agree |
| `bch_paritycheck` | B3 | — | 2 | 1 | 1 | — | `66702e0792ec3883` | agree |
| `bch_paritycheck` | N1 | — | 2 | 1 | 1 | — | `1e7a58690e4c6183` | agree |
| `bch_paritycheck` | N2 | — | 2 | 1 | 1 | — | `b8b036f86e7174d7` | agree |
| `bch_paritycheck` | N3 | — | 2 | 1 | 1 | — | `225baa5507f0982b` | agree |
| `bch_paritycheck` | N4 | — | 2 | 1 | 1 | — | `1983bdb7cd0c707a` | agree |
| `bch_paritycheck` | T2N-mother | — | 2 | 1 | 1 | — | `0247e7d83bdd8e89` | agree |
| `bch_paritycheck` | T2S-mother | — | 2 | 1 | 1 | — | `18ff215951a2bfdc` | agree |

## Scalar-fallback coverage

Detected kernel bundle: `avx2-pclmul`. Each row pairs a family's `scalar` arm with its detected arm at one row and batch; the digest comparison is the determinism group above.

| Row | B | Family | Arms | Scalar-arm median | Verdict |
|---|---|---|---|---|---|
| B1 | 1 | `bitslice-interleaved` | avx2-pclmul, scalar | 376.6 ns | covered |
| B1 | 1 | `clmul-fold` | avx2-pclmul, scalar | 81.19 ns | covered |
| B1 | 16 | `bitslice-interleaved` | avx2-pclmul, scalar | 798.9 ns | covered |
| B1 | 16 | `clmul-fold` | avx2-pclmul, scalar | 999.7 ns | covered |
| B1 | 256 | `bitslice-interleaved` | avx2-pclmul, scalar | 8.945 µs | covered |
| B1 | 256 | `clmul-fold` | avx2-pclmul, scalar | 16.33 µs | covered |
| B1 | 4096 | `bitslice-interleaved` | avx2-pclmul, scalar | 189.1 µs | covered |
| B1 | 4096 | `clmul-fold` | avx2-pclmul, scalar | 308 µs | covered |
| B2 | 1 | `bitslice-interleaved` | avx2-pclmul, scalar | 1.192 µs | covered |
| B2 | 1 | `clmul-fold` | avx2-pclmul, scalar | 306.8 ns | covered |
| B2 | 16 | `bitslice-interleaved` | avx2-pclmul, scalar | 3.823 µs | covered |
| B2 | 16 | `clmul-fold` | avx2-pclmul, scalar | 3.885 µs | covered |
| B2 | 256 | `bitslice-interleaved` | avx2-pclmul, scalar | 114.5 µs | covered |
| B2 | 256 | `clmul-fold` | avx2-pclmul, scalar | 130.2 µs | covered |
| B2 | 4096 | `bitslice-interleaved` | avx2-pclmul, scalar | 2.155 ms | covered |
| B2 | 4096 | `clmul-fold` | avx2-pclmul, scalar | 2.262 ms | covered |
| B3 | 1 | `bitslice-interleaved` | avx2-pclmul, scalar | 2.487 µs | covered |
| B3 | 1 | `clmul-fold` | avx2-pclmul, scalar | 612 ns | covered |
| B3 | 16 | `bitslice-interleaved` | avx2-pclmul, scalar | 7.694 µs | covered |
| B3 | 16 | `clmul-fold` | avx2-pclmul, scalar | 9.287 µs | covered |
| B3 | 256 | `bitslice-interleaved` | avx2-pclmul, scalar | 235.3 µs | covered |
| B3 | 256 | `clmul-fold` | avx2-pclmul, scalar | 264.5 µs | covered |
| B3 | 4096 | `bitslice-interleaved` | avx2-pclmul, scalar | 4.009 ms | covered |
| B3 | 4096 | `clmul-fold` | avx2-pclmul, scalar | 4.754 ms | covered |
| T2N-mother | 1 | `bitslice-interleaved` | avx2-pclmul, scalar | 2.402 ms | covered |
| T2N-mother | 1 | `clmul-fold` | avx2-pclmul, scalar | 323.8 µs | covered |
| T2N-mother | 16 | `bitslice-interleaved` | avx2-pclmul, scalar | 5.904 ms | covered |
| T2N-mother | 16 | `clmul-fold` | avx2-pclmul, scalar | 5.654 ms | covered |
| T2N-mother | 256 | `bitslice-interleaved` | avx2-pclmul, scalar | 69.31 ms | covered |
| T2N-mother | 256 | `clmul-fold` | avx2-pclmul, scalar | 90.14 ms | covered |
| T2N-mother | 4096 | `bitslice-interleaved` | avx2-pclmul, scalar | 1.111 s | covered |
| T2N-mother | 4096 | `clmul-fold` | avx2-pclmul, scalar | 1.447 s | covered |
| T2S-mother | 1 | `bitslice-interleaved` | avx2-pclmul, scalar | 579.6 µs | covered |
| T2S-mother | 1 | `clmul-fold` | avx2-pclmul, scalar | 51.2 µs | covered |
| T2S-mother | 16 | `bitslice-interleaved` | avx2-pclmul, scalar | 1.411 ms | covered |
| T2S-mother | 16 | `clmul-fold` | avx2-pclmul, scalar | 1.387 ms | covered |
| T2S-mother | 256 | `bitslice-interleaved` | avx2-pclmul, scalar | 17.05 ms | covered |
| T2S-mother | 256 | `clmul-fold` | avx2-pclmul, scalar | 22.19 ms | covered |
| T2S-mother | 4096 | `bitslice-interleaved` | avx2-pclmul, scalar | 274.7 ms | covered |
| T2S-mother | 4096 | `clmul-fold` | avx2-pclmul, scalar | 356.2 ms | covered |

## REQ-02 external-baseline comparison (aspirational, `W = 1`)

External medians are the committed survey trials (`4e732b56`, same host); gf2 is this run's Criterion median. W1 in information Mbit/s ($Bk/T$), W2 in matrix Mbit/s ($kn/T$). The aspirational target is gf2 at or above the strongest external cell; the outcome is reported either way.

| Cell | gf2 path | gf2 Mbit/s | Strongest external | External Mbit/s | gf2/external | Aspirational target |
|---|---|---|---|---|---|---|
| W1 B1 B=1 | `family=clmul-fold[avx2-pclmul]` | 86.16 | aff3ct `lfsr-scalar` | 111.5 | 0.773 | not met |
| W1 B1 B=16 | `family=clmul-fold[avx2-pclmul]` | 128.2 | aff3ct `lfsr-simd-inter` | 421.2 | 0.304 | not met |
| W1 B1 B=256 | `family=bitslice-interleaved[avx2-pclmul]` | 151.6 | aff3ct `lfsr-simd-inter` | 426.6 | 0.355 | not met |
| W1 B1 B=4096 | `family=bitslice-interleaved[avx2-pclmul]` | 113.9 | aff3ct `lfsr-simd-inter` | 413.4 | 0.275 | not met |
| W1 B2 B=1 | `family=clmul-fold[avx2-pclmul]` | 257.1 | bchlib `table-remainder` | 2,006 | 0.128 | not met |
| W1 B2 B=16 | `family=clmul-fold[avx2-pclmul]` | 346.9 | bchlib `table-remainder` | 2,231 | 0.156 | not met |
| W1 B2 B=256 | `family=bitslice-interleaved[avx2-pclmul]` | 151.3 | bchlib `table-remainder` | 2,257 | 0.067 | not met |
| W1 B2 B=4096 | `family=clmul-fold[avx2-pclmul]` | 130.2 | bchlib `table-remainder` | 2,269 | 0.0574 | not met |
| W1 B3 B=1 | `family=clmul-fold[avx2-pclmul]` | 560.7 | aff3ct `lfsr-scalar` | 111.9 | 5.01 | met |
| W1 B3 B=16 | `family=clmul-fold[avx2-pclmul]` | 633.5 | aff3ct `lfsr-simd-inter` | 435.5 | 1.45 | met |
| W1 B3 B=256 | `family=clmul-fold[avx2-pclmul]` | 278.8 | aff3ct `lfsr-simd-inter` | 429.6 | 0.649 | not met |
| W1 B3 B=4096 | `family=clmul-fold[avx2-pclmul]` | 238 | aff3ct `lfsr-simd-inter` | 430.4 | 0.553 | not met |
| W1 T2S B=1 | `route=shortened-restriction` | 44.19 | bchlib `table-remainder` | 4,598 | 0.00961 | not met |
| W1 T2S B=16 | `route=shortened-restriction` | 37.95 | bchlib `table-remainder` | 4,553 | 0.00833 | not met |
| W1 T2S B=256 | `route=shortened-restriction` | 37.85 | bchlib `table-remainder` | 4,587 | 0.00825 | not met |
| W1 T2S B=4096 | `route=shortened-restriction` | 37.87 | bchlib `table-remainder` | 4,581 | 0.00827 | not met |
| W1 T2N B=1 | `route=shortened-restriction` | 41.1 | bchlib `table-remainder` | 4,654 | 0.00883 | not met |
| W1 T2N B=16 | `route=shortened-restriction` | 40.03 | bchlib `table-remainder` | 4,676 | 0.00856 | not met |
| W1 T2N B=256 | `route=shortened-restriction` | 40.01 | bchlib `table-remainder` | 4,678 | 0.00855 | not met |
| W1 T2N B=4096 | `route=shortened-restriction` | 39.97 | bchlib `table-remainder` | 4,684 | 0.00853 | not met |
| W2 B1 | `materialize` | 1,650 | aff3ct `basis-encode-pack` | 297.7 | 5.54 | met |
| W2 B2 | `materialize` | 16,646 | m4ri `genmatrix-rref` | 1,067 | 15.6 | met |
| W2 B3 | `materialize` | 37,265 | m4ri `genmatrix-rref` | 1,464 | 25.4 | met |
| W2 T2S | `materialize` | 27.95 | m4ri `genmatrix-rref` | 1,345 | 0.0208 | not met |
| W2 T2N | `materialize` | 23.45 | m4ri `genmatrix-rref` | 392 | 0.0598 | not met |

Aspirational outcome: target met on 5 of 25 comparable cells.

## Every measured cell

| Benchmark ID | Samples | Sampling | Median (ns) | 95% CI lower (ns) | 95% CI upper (ns) | M units/s |
|---|---|---|---|---|---|---|
| `bch_batch_decode/1` | 100 | Linear | 24653.08 | 24649.11 | 24658.32 | — |
| `bch_batch_decode/10` | 100 | Flat | 11304249.00 | 11302274.00 | 11306690.00 | — |
| `bch_batch_decode/100` | 100 | Flat | 124196413.00 | 124173057.00 | 124233358.00 | — |
| `bch_batch_decode/50` | 100 | Flat | 61482595.50 | 61473896.00 | 61495676.00 | — |
| `bch_encode_pns_16383_16215/1` | 100 | Linear | 63711.10 | 63676.67 | 63761.27 | 254.5 |
| `bch_encode_pns_16383_16215/10` | 100 | Linear | 638818.05 | 638645.86 | 639026.20 | 253.8 |
| `bch_encode_pns_16383_16215/100` | 100 | Flat | 6398446.56 | 6396941.62 | 6401894.12 | 253.4 |
| `bch_encode_pns_16383_16215/50` | 100 | Flat | 3201762.50 | 3201252.16 | 3202709.69 | 253.2 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=1` | 100 | Linear | 234.91 | 234.82 | 235.07 | 21.28 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=16` | 100 | Linear | 654.41 | 653.86 | 655.12 | 122.2 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=256` | 100 | Linear | 8443.54 | 8439.90 | 8446.15 | 151.6 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=4096` | 100 | Linear | 179806.44 | 179769.81 | 179861.15 | 113.9 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=1` | 100 | Linear | 910.36 | 908.65 | 911.00 | 70.3 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=16` | 100 | Linear | 3574.94 | 3572.24 | 3576.83 | 286.4 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=256` | 100 | Linear | 108311.53 | 108195.56 | 108368.38 | 151.3 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=4096` | 100 | Flat | 2151365.50 | 2150425.29 | 2151924.88 | 121.9 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=1` | 100 | Linear | 2139.02 | 2138.39 | 2141.47 | 104.3 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=16` | 100 | Linear | 7316.60 | 7314.24 | 7319.53 | 487.7 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=256` | 100 | Linear | 234468.01 | 234108.84 | 234586.43 | 243.5 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=4096` | 100 | Flat | 3980843.35 | 3980299.92 | 3981574.12 | 229.5 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=1` | 10 | Flat | 1650175.11 | 1649540.05 | 1652339.33 | 39.6 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=16` | 10 | Flat | 5150684.79 | 5148720.18 | 5152441.94 | 203 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=256` | 10 | Flat | 66252092.81 | 66191962.50 | 66277300.44 | 252.5 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 1064474197.50 | 1062722994.00 | 1066236455.50 | 251.4 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=1` | 10 | Flat | 445545.00 | 445360.91 | 445850.09 | 36.39 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=16` | 10 | Flat | 1244226.30 | 1243971.81 | 1244626.84 | 208.5 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=256` | 10 | Flat | 16398425.87 | 16394887.63 | 16412017.87 | 253.1 |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 263193061.25 | 263077350.75 | 263369327.25 | 252.3 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=1` | 100 | Linear | 376.61 | 376.43 | 376.76 | 13.28 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=16` | 100 | Linear | 798.87 | 798.33 | 799.48 | 100.1 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=256` | 100 | Linear | 8945.12 | 8941.13 | 8948.36 | 143.1 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=4096` | 100 | Linear | 189053.84 | 188994.60 | 189124.77 | 108.3 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=1` | 100 | Linear | 1191.65 | 1191.40 | 1191.90 | 53.71 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=16` | 100 | Linear | 3822.83 | 3821.18 | 3825.38 | 267.9 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=256` | 100 | Linear | 114476.67 | 114358.45 | 114529.29 | 143.1 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=4096` | 100 | Flat | 2154585.50 | 2153818.00 | 2155252.05 | 121.7 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=1` | 100 | Linear | 2487.32 | 2484.30 | 2490.12 | 89.65 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=16` | 100 | Linear | 7694.07 | 7691.13 | 7700.85 | 463.7 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=256` | 100 | Linear | 235345.14 | 234964.38 | 235480.74 | 242.6 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=4096` | 100 | Flat | 4009358.85 | 4007910.73 | 4019005.81 | 227.8 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=1` | 10 | Flat | 2402201.88 | 2399188.83 | 2404974.81 | 27.2 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=16` | 10 | Flat | 5903757.69 | 5900267.80 | 5904957.46 | 177.1 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=256` | 10 | Flat | 69305958.75 | 69197275.75 | 69400571.06 | 241.4 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 1111291692.00 | 1110629358.50 | 1112482278.00 | 240.8 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=1` | 10 | Flat | 579576.55 | 579295.64 | 579893.64 | 27.98 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=16` | 10 | Flat | 1411054.06 | 1410241.80 | 1412526.91 | 183.9 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=256` | 10 | Flat | 17049182.12 | 17040047.57 | 17055057.63 | 243.5 |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 274724854.00 | 273758048.50 | 275779784.50 | 241.8 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=1` | 100 | Linear | 58.03 | 57.99 | 58.12 | 86.16 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=16` | 100 | Linear | 624.07 | 623.68 | 624.88 | 128.2 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=256` | 100 | Linear | 9875.63 | 9873.99 | 9878.14 | 129.6 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=4096` | 100 | Linear | 202087.11 | 201906.30 | 202203.17 | 101.3 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=1` | 100 | Linear | 248.97 | 248.85 | 249.13 | 257.1 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=16` | 100 | Linear | 2951.57 | 2950.67 | 2952.56 | 346.9 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=256` | 100 | Linear | 112442.22 | 112380.60 | 112468.02 | 145.7 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=4096` | 100 | Flat | 2013262.60 | 2012512.00 | 2013677.64 | 130.2 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=1` | 100 | Linear | 397.71 | 397.10 | 398.36 | 560.7 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=16` | 100 | Linear | 5632.55 | 5631.76 | 5634.48 | 633.5 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=256` | 100 | Linear | 204799.01 | 204734.71 | 204918.02 | 278.8 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=4096` | 100 | Flat | 3837828.73 | 3836490.27 | 3838893.35 | 238 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=1` | 10 | Flat | 204764.79 | 204604.55 | 205077.46 | 319.1 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=16` | 10 | Flat | 3747610.64 | 3744749.92 | 3752479.92 | 279 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=256` | 10 | Flat | 59797950.00 | 59732371.44 | 59817855.11 | 279.7 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 962408031.50 | 961851167.50 | 963168885.00 | 278.1 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=1` | 10 | Flat | 23125.61 | 23106.59 | 23166.82 | 701.2 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=16` | 10 | Flat | 935080.89 | 934591.99 | 936665.01 | 277.5 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=256` | 10 | Flat | 15061145.21 | 15055260.29 | 15072908.06 | 275.6 |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 240653541.83 | 240534507.67 | 240771507.50 | 276 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=1` | 100 | Linear | 81.19 | 81.01 | 81.32 | 61.59 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=16` | 100 | Linear | 999.70 | 998.90 | 1000.49 | 80.02 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=256` | 100 | Linear | 16329.59 | 16324.02 | 16334.47 | 78.39 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=4096` | 100 | Linear | 308012.28 | 307925.74 | 308046.49 | 66.49 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=1` | 100 | Linear | 306.79 | 306.64 | 306.89 | 208.6 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=16` | 100 | Linear | 3884.75 | 3883.32 | 3885.63 | 263.6 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=256` | 100 | Linear | 130167.22 | 130117.91 | 130209.12 | 125.9 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=4096` | 100 | Flat | 2262108.87 | 2261638.65 | 2262908.65 | 115.9 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=1` | 100 | Linear | 612.00 | 611.14 | 612.86 | 364.4 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=16` | 100 | Linear | 9286.94 | 9283.28 | 9288.92 | 384.2 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=256` | 100 | Linear | 264455.28 | 264124.39 | 264882.07 | 215.9 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=4096` | 100 | Flat | 4754409.64 | 4753291.00 | 4756969.68 | 192.1 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=1` | 10 | Flat | 323808.35 | 323604.23 | 324023.70 | 201.8 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=16` | 10 | Flat | 5654162.81 | 5649926.59 | 5660013.01 | 184.9 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=256` | 10 | Flat | 90137969.08 | 90080529.58 | 90270063.83 | 185.6 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 1446630194.00 | 1443345956.00 | 1448523448.00 | 185 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=1` | 10 | Flat | 51198.24 | 51135.57 | 51336.07 | 316.7 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=16` | 10 | Flat | 1386530.55 | 1385425.04 | 1387821.31 | 187.1 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=256` | 10 | Flat | 22193926.65 | 22169913.96 | 22225257.07 | 187 |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 356228460.50 | 356135017.50 | 356495712.25 | 186.4 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=1` | 100 | Linear | 67.17 | 67.08 | 67.31 | 74.44 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=16` | 100 | Linear | 651.51 | 650.79 | 652.75 | 122.8 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=256` | 100 | Linear | 10154.16 | 10148.14 | 10219.29 | 126.1 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=4096` | 100 | Linear | 243244.31 | 243183.99 | 243282.80 | 84.2 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=1` | 100 | Linear | 380.50 | 380.31 | 380.65 | 168.2 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=16` | 100 | Linear | 4738.13 | 4737.30 | 4739.70 | 216.1 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=256` | 100 | Linear | 177005.47 | 176843.31 | 177104.05 | 92.56 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=4096` | 100 | Flat | 3137659.34 | 3136481.53 | 3138471.53 | 83.55 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=1` | 100 | Linear | 829.68 | 829.30 | 829.96 | 268.8 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=16` | 100 | Linear | 11874.74 | 11872.78 | 11879.00 | 300.5 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=256` | 100 | Linear | 442549.15 | 442404.00 | 442694.02 | 129 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=4096` | 100 | Flat | 7644893.14 | 7640366.71 | 7647536.86 | 119.5 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=1` | 100 | Linear | 101.11 | 101.01 | 101.20 | 39.56 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=16` | 100 | Linear | 1285.92 | 1285.41 | 1286.45 | 49.77 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=256` | 100 | Linear | 20457.95 | 20454.65 | 20462.87 | 50.05 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=4096` | 100 | Linear | 325951.59 | 325900.24 | 326067.01 | 50.27 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=1` | 100 | Linear | 362.29 | 361.91 | 362.58 | 60.72 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=16` | 100 | Linear | 5466.25 | 5463.95 | 5469.13 | 64.4 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=256` | 100 | Linear | 87959.86 | 87933.16 | 88008.66 | 64.03 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=4096` | 100 | Linear | 1407104.18 | 1406894.57 | 1407804.97 | 64.04 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=1` | 100 | Linear | 1528.26 | 1504.93 | 1529.56 | 3.926 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=16` | 100 | Linear | 23927.73 | 23920.28 | 23934.01 | 4.012 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=256` | 100 | Linear | 449103.88 | 448898.87 | 449294.53 | 3.42 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=4096` | 100 | Flat | 7703084.21 | 7694382.71 | 7706662.71 | 3.19 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=1` | 100 | Linear | 87734.37 | 87721.21 | 87758.77 | 2.542 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=16` | 100 | Linear | 1410351.31 | 1409817.14 | 1411130.33 | 2.53 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=256` | 100 | Flat | 22578235.33 | 22571286.67 | 22584947.00 | 2.528 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=4096` | 100 | Flat | 360797091.00 | 360704359.00 | 360831235.50 | 2.532 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=1` | 10 | Flat | 619171.81 | 618622.03 | 619996.55 | 105.5 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=16` | 10 | Flat | 10336225.18 | 10330280.87 | 10339928.36 | 101.1 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=256` | 10 | Flat | 165172325.00 | 165100593.38 | 165282739.38 | 101.3 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 2649361933.50 | 2648844961.00 | 2650156273.00 | 101 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=1` | 10 | Flat | 136117.16 | 135856.43 | 136242.76 | 119.1 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=16` | 10 | Flat | 2572358.90 | 2570677.31 | 2575169.66 | 100.9 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=256` | 10 | Flat | 41188394.12 | 41164327.85 | 41208163.46 | 100.8 |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 660687917.50 | 660286480.50 | 661217791.00 | 100.5 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=1` | 100 | Linear | 264.88 | 264.64 | 265.13 | 241.6 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=16` | 100 | Linear | 3083.49 | 3082.82 | 3084.30 | 332.1 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=256` | 100 | Linear | 114963.92 | 114902.31 | 115051.55 | 142.5 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=4096` | 100 | Flat | 2047185.76 | 2046980.20 | 2047507.78 | 128.1 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=1` | 100 | Linear | 505.48 | 505.19 | 505.76 | 441.2 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=16` | 100 | Linear | 7115.47 | 7114.33 | 7116.94 | 501.4 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=256` | 100 | Linear | 261881.39 | 261756.80 | 262025.25 | 218 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=4096` | 100 | Flat | 4484034.92 | 4482242.83 | 4487712.46 | 203.7 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=1` | 10 | Flat | 257988.02 | 257763.01 | 258077.81 | 253.3 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=16` | 10 | Flat | 4558102.02 | 4553211.90 | 4562094.50 | 229.4 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=256` | 10 | Flat | 72696821.14 | 72656621.64 | 72713807.71 | 230.1 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 1175739070.50 | 1175332529.00 | 1176900567.00 | 227.6 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=1` | 10 | Flat | 36286.55 | 36276.62 | 36300.60 | 446.9 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=16` | 10 | Flat | 1135048.14 | 1134528.35 | 1135887.28 | 228.6 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=256` | 10 | Flat | 18262752.62 | 18247062.71 | 18288854.89 | 227.3 |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 292582649.50 | 292357548.50 | 293002134.75 | 227 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=1` | 10 | Flat | 796995.48 | 796257.97 | 798027.55 | 40.41 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=16` | 10 | Flat | 13032743.12 | 13002656.81 | 13045183.82 | 39.54 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=256` | 10 | Flat | 209734403.00 | 209546712.00 | 209856707.00 | 39.31 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=4096` | 10 | Flat | 3361629917.00 | 3359950010.00 | 3363402909.00 | 39.24 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=1` | 10 | Flat | 160455.52 | 160188.30 | 160664.93 | 43.83 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=16` | 10 | Flat | 2972399.49 | 2969048.62 | 2975777.85 | 37.85 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=256` | 10 | Flat | 47664721.27 | 47621551.05 | 47670974.05 | 37.77 |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=4096` | 10 | Flat | 762481639.00 | 761787330.00 | 763041187.00 | 37.78 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=1` | 10 | Flat | 783609.79 | 782892.77 | 784062.01 | 41.1 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=16` | 10 | Flat | 12874282.81 | 12866626.23 | 13012698.28 | 40.03 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=256` | 10 | Flat | 206104692.50 | 205999346.83 | 206229629.83 | 40.01 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=4096` | 10 | Flat | 3300449563.50 | 3298025499.00 | 3301285443.00 | 39.97 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=1` | 10 | Flat | 159139.69 | 158104.34 | 159244.87 | 44.19 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=16` | 10 | Flat | 2964901.05 | 2963247.02 | 2966835.73 | 37.95 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=256` | 10 | Flat | 47560805.77 | 47505665.00 | 47629088.91 | 37.85 |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=4096` | 10 | Flat | 760582599.50 | 759852280.50 | 760834506.00 | 37.87 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=1` | 100 | Linear | 87.22 | 87.18 | 87.26 | 57.32 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=16` | 100 | Linear | 936.46 | 935.86 | 937.05 | 85.43 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=256` | 100 | Linear | 15470.57 | 15467.88 | 15472.52 | 82.74 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=4096` | 100 | Linear | 332429.42 | 332346.49 | 332546.67 | 61.61 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=1` | 100 | Linear | 382.78 | 382.63 | 382.88 | 167.2 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=16` | 100 | Linear | 4856.78 | 4855.13 | 4857.95 | 210.8 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=256` | 100 | Linear | 178790.87 | 178723.53 | 178903.63 | 91.64 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=4096` | 100 | Flat | 3214262.56 | 3212501.28 | 3215576.31 | 81.56 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=1` | 100 | Linear | 817.83 | 817.58 | 817.99 | 272.7 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=16` | 100 | Linear | 12810.63 | 12794.44 | 12834.42 | 278.5 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=256` | 100 | Linear | 448755.69 | 448391.93 | 449217.36 | 127.2 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=4096` | 100 | Flat | 7729687.29 | 7725767.29 | 7733528.71 | 118.2 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=1` | 100 | Linear | 114.00 | 113.94 | 114.07 | 35.09 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=16` | 100 | Linear | 1415.40 | 1414.45 | 1416.85 | 45.22 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=256` | 100 | Linear | 25268.92 | 25260.38 | 25278.75 | 40.52 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=4096` | 100 | Linear | 403075.81 | 402891.37 | 403288.08 | 40.65 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=1` | 100 | Linear | 380.57 | 380.40 | 380.77 | 57.81 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=16` | 100 | Linear | 5623.98 | 5622.44 | 5627.69 | 62.59 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=256` | 100 | Linear | 93024.97 | 93007.88 | 93051.95 | 60.54 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=4096` | 100 | Linear | 1490893.53 | 1490512.15 | 1491486.64 | 60.44 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=1` | 100 | Linear | 1781.82 | 1780.88 | 1782.48 | 3.367 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=16` | 100 | Linear | 28355.25 | 28348.72 | 28371.99 | 3.386 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=256` | 100 | Linear | 544151.98 | 539788.99 | 548713.95 | 2.823 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=4096` | 100 | Flat | 8524327.42 | 8320443.17 | 8885096.75 | 2.883 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=1` | 100 | Linear | 88572.40 | 88529.34 | 88629.45 | 2.518 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=16` | 100 | Linear | 1423637.20 | 1423498.97 | 1424111.12 | 2.506 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=256` | 100 | Flat | 22783404.67 | 22778941.33 | 22788504.67 | 2.506 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=4096` | 100 | Flat | 371140939.00 | 371045554.00 | 371318615.00 | 2.461 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=1` | 10 | Flat | 618940.47 | 618429.04 | 619801.42 | 105.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=16` | 10 | Flat | 10339007.65 | 10331887.31 | 10343728.29 | 101.1 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=256` | 10 | Flat | 166518670.75 | 166437697.88 | 166605678.75 | 100.5 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=4096` | 10 | Flat | 2661754753.00 | 2660539962.00 | 2663718549.00 | 100.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=1` | 10 | Flat | 136081.75 | 135975.54 | 136302.52 | 119.2 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=16` | 10 | Flat | 2571612.15 | 2571041.23 | 2572426.50 | 100.9 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=256` | 10 | Flat | 41209965.85 | 41177232.96 | 41228258.15 | 100.7 |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=4096` | 10 | Flat | 662203591.00 | 662022570.00 | 663491022.50 | 100.3 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=1` | 100 | Linear | 1368.49 | 1368.17 | 1368.77 | 3.654 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=16` | 100 | Linear | 8782.31 | 8739.57 | 8825.22 | 9.109 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=256` | 100 | Linear | 20836.29 | 20792.59 | 20868.43 | 61.43 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=4096` | 100 | Linear | 216885.76 | 216244.46 | 218694.46 | 94.43 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=1` | 100 | Linear | 26196.85 | 26192.42 | 26206.42 | 2.443 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=16` | 100 | Linear | 42590.19 | 42466.87 | 42772.24 | 24.04 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=256` | 100 | Linear | 70057.73 | 69604.98 | 70289.25 | 233.9 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=4096` | 100 | Linear | 733391.25 | 732504.81 | 734667.45 | 357.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=1` | 100 | Linear | 26291.01 | 26281.76 | 26299.15 | 8.482 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=16` | 100 | Linear | 42295.34 | 42274.51 | 42371.44 | 84.36 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=256` | 100 | Linear | 127966.75 | 127299.74 | 128284.00 | 446.1 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=4096` | 100 | Linear | 1548264.74 | 1543646.57 | 1552008.55 | 590 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=1` | 100 | Linear | 290.37 | 290.21 | 290.42 | 13.78 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=16` | 100 | Linear | 7067.20 | 7057.39 | 7083.01 | 9.056 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=256` | 100 | Linear | 21755.13 | 21728.55 | 21788.14 | 47.07 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=4096` | 100 | Linear | 247167.80 | 246601.83 | 247729.22 | 66.29 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=1` | 100 | Linear | 701.36 | 701.17 | 701.49 | 31.37 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=16` | 100 | Linear | 8240.13 | 8159.27 | 8288.50 | 42.72 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=256` | 100 | Linear | 52884.81 | 52851.13 | 52915.77 | 106.5 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=4096` | 100 | Linear | 735089.92 | 732829.68 | 736972.15 | 122.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=1` | 100 | Linear | 3070.75 | 3066.54 | 3072.77 | 1.954 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=16` | 100 | Linear | 60663.89 | 60586.58 | 60755.38 | 1.582 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=256` | 100 | Linear | 825394.62 | 825113.02 | 825932.17 | 1.861 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=4096` | 100 | Flat | 12978312.12 | 12972674.75 | 12990798.62 | 1.894 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=1` | 100 | Linear | 90799.29 | 90784.06 | 90812.87 | 2.456 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=16` | 100 | Flat | 9002685.00 | 8993460.00 | 9015666.67 | 0.3963 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=256` | 100 | Flat | 142953410.50 | 142836515.50 | 143022917.00 | 0.3993 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=4096` | 100 | Flat | 2288859110.00 | 2288475114.00 | 2289288234.50 | 0.3991 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=1` | 10 | Flat | 653527.16 | 653175.68 | 654139.17 | 99.99 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=16` | 10 | Flat | 2085572.69 | 2077773.17 | 2091840.53 | 501.3 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=256` | 10 | Flat | 29997103.53 | 29969026.91 | 30004117.38 | 557.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=4096` | 10 | Flat | 475019426.00 | 474715874.00 | 475403112.75 | 563.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=1` | 10 | Flat | 171014.86 | 170904.82 | 171258.64 | 94.82 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=16` | 10 | Flat | 545804.90 | 543969.52 | 547054.71 | 475.3 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=256` | 10 | Flat | 7289264.57 | 7280522.42 | 7347526.90 | 569.5 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=4096` | 10 | Flat | 118406556.20 | 118340436.00 | 118516166.70 | 560.9 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=1` | 100 | Linear | 72.23 | 72.21 | 72.30 | 69.22 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=16` | 100 | Linear | 6261.78 | 6248.11 | 6276.17 | 12.78 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=256` | 100 | Linear | 10811.18 | 10807.59 | 10817.20 | 118.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=4096` | 100 | Linear | 88049.32 | 87904.58 | 88175.29 | 232.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=1` | 100 | Linear | 382.19 | 381.96 | 382.36 | 167.5 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=16` | 100 | Linear | 7043.86 | 7041.37 | 7047.54 | 145.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=256` | 100 | Linear | 82629.94 | 82531.49 | 82898.16 | 198.3 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=4096` | 100 | Linear | 593879.01 | 591749.34 | 595253.52 | 441.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=1` | 100 | Linear | 805.34 | 804.91 | 805.75 | 276.9 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=16` | 100 | Linear | 9487.09 | 9345.12 | 9908.83 | 376.1 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=256` | 100 | Linear | 89519.41 | 89383.85 | 89681.31 | 637.7 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=4096` | 100 | Linear | 1433912.18 | 1427670.52 | 1442123.56 | 637 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=1` | 100 | Linear | 110.27 | 110.17 | 110.38 | 36.27 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=16` | 100 | Linear | 6435.83 | 6419.04 | 6452.13 | 9.944 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=256` | 100 | Linear | 14731.32 | 14713.91 | 14750.52 | 69.51 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=4096` | 100 | Linear | 155136.48 | 154953.32 | 155373.95 | 105.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=1` | 100 | Linear | 375.99 | 375.84 | 376.28 | 58.51 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=16` | 100 | Linear | 7146.40 | 7143.66 | 7147.97 | 49.26 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=256` | 100 | Linear | 43513.71 | 43446.46 | 43566.32 | 129.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=4096` | 100 | Linear | 548977.06 | 546577.28 | 551033.27 | 164.1 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=1` | 100 | Linear | 1548.76 | 1548.57 | 1549.48 | 3.874 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=16` | 100 | Linear | 52248.94 | 52140.35 | 52312.65 | 1.837 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=256` | 100 | Linear | 753315.20 | 753079.72 | 753605.11 | 2.039 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=4096` | 100 | Flat | 11904811.50 | 11899613.60 | 11910021.60 | 2.064 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=1` | 100 | Linear | 87705.92 | 87691.11 | 87722.17 | 2.543 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=16` | 100 | Flat | 8957253.83 | 8948768.17 | 8968953.17 | 0.3983 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=256` | 100 | Flat | 142366063.00 | 142236782.00 | 142455893.50 | 0.401 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=4096` | 100 | Flat | 2275489335.00 | 2275082775.50 | 2275978407.00 | 0.4014 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=1` | 10 | Flat | 619062.66 | 618025.45 | 619704.27 | 105.6 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=16` | 10 | Flat | 2159677.00 | 2144361.84 | 2172076.94 | 484.1 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=256` | 10 | Flat | 28999553.59 | 28981228.18 | 30309085.71 | 576.8 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=4096` | 10 | Flat | 460122028.00 | 460003304.50 | 460489510.00 | 581.7 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=1` | 10 | Flat | 135828.34 | 135652.76 | 136182.46 | 119.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=16` | 10 | Flat | 529034.76 | 525919.85 | 530029.14 | 490.4 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=256` | 10 | Flat | 7551021.80 | 7500661.41 | 7693937.83 | 549.7 |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=4096` | 10 | Flat | 114467722.70 | 114391175.40 | 114569426.10 | 580.2 |
| `bch_genmatrix_w2/materialize/fresh-alloc/B1` | 100 | Linear | 45.47 | 45.45 | 45.49 | 1,650 |
| `bch_genmatrix_w2/materialize/fresh-alloc/B2` | 100 | Linear | 488.27 | 488.10 | 488.52 | 16,646 |
| `bch_genmatrix_w2/materialize/fresh-alloc/B3` | 100 | Linear | 1525.95 | 1525.65 | 1526.31 | 37,265 |
| `bch_genmatrix_w2/materialize/fresh-alloc/N1` | 100 | Linear | 82.33 | 82.21 | 82.39 | 631.6 |
| `bch_genmatrix_w2/materialize/fresh-alloc/N2` | 100 | Linear | 583.51 | 583.08 | 584.21 | 1,169 |
| `bch_genmatrix_w2/materialize/fresh-alloc/N3` | 100 | Linear | 3672.77 | 3671.55 | 3674.81 | 16.34 |
| `bch_genmatrix_w2/materialize/fresh-alloc/N4` | 100 | Linear | 570041.12 | 569889.14 | 570200.30 | 99.76 |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2N` | 10 | Flat | 44508902125.50 | 44241070728.00 | 44621680639.00 | 23.45 |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2N-mother` | 10 | Linear | 49657132.04 | 49486187.00 | 49748988.44 | 86,236 |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2S` | 10 | Flat | 1811391200.50 | 1809832238.00 | 1813934719.00 | 27.95 |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2S-mother` | 10 | Linear | 2793157.78 | 2790079.60 | 2805436.16 | 95,108 |
| `bch_genmatrix_w2/materialize/warm-reuse/B1` | 100 | Linear | 37.37 | 37.29 | 37.47 | 2,007 |
| `bch_genmatrix_w2/materialize/warm-reuse/B2` | 100 | Linear | 476.35 | 476.17 | 476.50 | 17,063 |
| `bch_genmatrix_w2/materialize/warm-reuse/B3` | 100 | Linear | 1465.67 | 1465.26 | 1465.93 | 38,798 |
| `bch_genmatrix_w2/materialize/warm-reuse/N1` | 100 | Linear | 71.28 | 71.23 | 71.39 | 729.5 |
| `bch_genmatrix_w2/materialize/warm-reuse/N2` | 100 | Linear | 483.55 | 482.81 | 484.03 | 1,410 |
| `bch_genmatrix_w2/materialize/warm-reuse/N3` | 100 | Linear | 1883.07 | 1881.68 | 1884.02 | 31.86 |
| `bch_genmatrix_w2/materialize/warm-reuse/N4` | 100 | Linear | 346144.63 | 346029.80 | 346267.66 | 164.3 |
| `bch_genmatrix_w2/materialize/warm-reuse/T2N` | 10 | Flat | 43814635967.00 | 43760454582.00 | 43847491077.00 | 23.82 |
| `bch_genmatrix_w2/materialize/warm-reuse/T2N-mother` | 10 | Linear | 27396174.05 | 27364107.42 | 27418442.64 | 156,308 |
| `bch_genmatrix_w2/materialize/warm-reuse/T2S` | 10 | Flat | 1809052323.50 | 1808196449.00 | 1810262100.00 | 27.99 |
| `bch_genmatrix_w2/materialize/warm-reuse/T2S-mother` | 10 | Linear | 753906.91 | 752112.77 | 761606.30 | 352,365 |
| `bch_genmatrix_w2/reference/fresh-alloc/B1` | 100 | Linear | 454.74 | 453.45 | 466.02 | 164.9 |
| `bch_genmatrix_w2/reference/fresh-alloc/B2` | 100 | Linear | 41391.59 | 41377.15 | 41417.23 | 196.4 |
| `bch_genmatrix_w2/reference/fresh-alloc/B3` | 100 | Linear | 294256.19 | 293063.53 | 294781.30 | 193.2 |
| `bch_genmatrix_w2/reference/fresh-alloc/N1` | 100 | Linear | 350.47 | 350.35 | 350.61 | 148.4 |
| `bch_genmatrix_w2/reference/fresh-alloc/N2` | 100 | Linear | 6673.32 | 6672.21 | 6675.68 | 102.2 |
| `bch_genmatrix_w2/reference/fresh-alloc/N3` | 100 | Linear | 14058.26 | 14053.52 | 14063.32 | 4.268 |
| `bch_genmatrix_w2/reference/fresh-alloc/N4` | 100 | Flat | 25348791.25 | 25342068.50 | 25352303.50 | 2.243 |
| `bch_genmatrix_w2/reference/fresh-alloc/T2N-mother` | 10 | Flat | 34485028664.50 | 34479347991.50 | 34490786322.00 | 124.2 |
| `bch_genmatrix_w2/reference/fresh-alloc/T2S-mother` | 10 | Flat | 2045272688.50 | 2043224942.50 | 2046115313.50 | 129.9 |
| `bch_genmatrix_w2/reference/warm-reuse/B1` | 100 | Linear | 450.91 | 450.53 | 451.36 | 166.3 |
| `bch_genmatrix_w2/reference/warm-reuse/B2` | 100 | Linear | 41382.42 | 41367.88 | 41400.19 | 196.4 |
| `bch_genmatrix_w2/reference/warm-reuse/B3` | 100 | Linear | 292618.46 | 292533.06 | 292749.23 | 194.3 |
| `bch_genmatrix_w2/reference/warm-reuse/N1` | 100 | Linear | 341.61 | 341.45 | 341.76 | 152.2 |
| `bch_genmatrix_w2/reference/warm-reuse/N2` | 100 | Linear | 6562.60 | 6561.29 | 6565.41 | 103.9 |
| `bch_genmatrix_w2/reference/warm-reuse/N3` | 100 | Linear | 16629.96 | 16623.06 | 16634.18 | 3.608 |
| `bch_genmatrix_w2/reference/warm-reuse/N4` | 100 | Flat | 25137090.00 | 25128887.50 | 25143427.50 | 2.262 |
| `bch_genmatrix_w2/reference/warm-reuse/T2N-mother` | 10 | Flat | 34355719374.50 | 34352511188.50 | 34358257616.50 | 124.6 |
| `bch_genmatrix_w2/reference/warm-reuse/T2S-mother` | 10 | Flat | 2038465859.00 | 2037730510.00 | 2039785820.00 | 130.3 |
| `bch_paritycheck/materialize/warm-reuse/B1` | 100 | Linear | 90.77 | 90.73 | 90.89 | 1,653 |
| `bch_paritycheck/materialize/warm-reuse/B2` | 100 | Linear | 5602.44 | 5598.84 | 5605.16 | 1,428 |
| `bch_paritycheck/materialize/warm-reuse/B3` | 100 | Linear | 10012.61 | 10002.95 | 10019.01 | 815 |
| `bch_paritycheck/materialize/warm-reuse/N1` | 100 | Linear | 68.58 | 68.51 | 68.68 | 1,706 |
| `bch_paritycheck/materialize/warm-reuse/N2` | 100 | Linear | 327.99 | 327.77 | 328.19 | 850.6 |
| `bch_paritycheck/materialize/warm-reuse/N3` | 100 | Linear | 1779.61 | 1775.90 | 1781.27 | 22.48 |
| `bch_paritycheck/materialize/warm-reuse/N4` | 100 | Linear | 172189.19 | 172162.49 | 172244.34 | 47.39 |
| `bch_paritycheck/materialize/warm-reuse/T2N-mother` | 10 | Linear | 78972544.55 | 78948547.14 | 79053196.46 | 159.3 |
| `bch_paritycheck/materialize/warm-reuse/T2S-mother` | 10 | Linear | 12048378.57 | 12044361.51 | 12055944.44 | 228.4 |
| `bch_paritycheck/reference/warm-reuse/B1` | 100 | Linear | 597.15 | 596.84 | 597.71 | 251.2 |
| `bch_paritycheck/reference/warm-reuse/B2` | 100 | Linear | 38426.36 | 38403.05 | 38442.07 | 208.2 |
| `bch_paritycheck/reference/warm-reuse/B3` | 100 | Linear | 168757.73 | 168662.73 | 168948.15 | 48.35 |
| `bch_paritycheck/reference/warm-reuse/N1` | 100 | Linear | 380.22 | 379.99 | 380.38 | 307.7 |
| `bch_paritycheck/reference/warm-reuse/N2` | 100 | Linear | 6607.45 | 6603.73 | 6614.20 | 42.23 |
| `bch_paritycheck/reference/warm-reuse/N3` | 100 | Linear | 15264.46 | 15253.87 | 15271.27 | 2.62 |
| `bch_paritycheck/reference/warm-reuse/N4` | 100 | Flat | 24775142.00 | 24772070.50 | 24777623.67 | 0.3294 |
| `bch_paritycheck/reference/warm-reuse/T2N-mother` | 10 | Flat | 24426955446.00 | 24422131962.00 | 24434567401.00 | 0.5151 |
| `bch_paritycheck/reference/warm-reuse/T2S-mother` | 10 | Flat | 1424881928.00 | 1423505265.50 | 1426042250.00 | 1.932 |
| `bch_sequential_vs_batch/batch_operation` | 100 | Linear | 6450.27 | 6444.65 | 6460.07 | 170.5 |
| `bch_sequential_vs_batch/sequential_loop` | 100 | Linear | 9044.06 | 9040.31 | 9046.86 | 121.6 |
| `bch_single_vs_batch/decode_into_loop` | 100 | Flat | 61573371.00 | 61555836.00 | 61580347.00 | — |
| `bch_single_vs_batch/single_loop` | 100 | Flat | 61495205.50 | 61487005.50 | 61509016.00 | — |

## Pinned inputs

| File | SHA-256 | Bytes |
|---|---|---|
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/host.txt` | `decf6416155f487a1cd836b08d18c5e49dfd3ff70816fa342ef980e24b6c6f64` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/dispatch.jsonl` | `fdb17a72bfa6cc13e1293b5e3cfb15c21aed0b1c520ca3f13f4fe3f58e785f7d` | 118212 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/logs/batch_operations-criterion.txt` | `7a5670f7be90bf12969374033c7dfd6910d412ec0766a69daa24eed747269573` | 4160 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/logs/bch_parallel-criterion.txt` | `15d776f4861f426b0bb05e63c30f310e187763bc0512db53553bb9736a824f76` | 4147 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/logs/bch_encode_w1-criterion.txt` | `dead9e07def5fd5010a2e88eeb84937281840a0054f739bb70b44506b3b0a8b7` | 186959 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/logs/bch_genmatrix-criterion.txt` | `38a93e320d6eb1d54069dae423a52db8aaec1c85f50cef4b996323a2137a9187` | 35239 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__1/benchmark.json` | `9845e3a0638c5254e4de86192737de2436fb7347d3738297f0f2072e5056625d` | 192 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__1/estimates.json` | `18de24f32539d2a1de3e5ddba78e94f7c48971a98d236424d0b1125e778f41e1` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__1/sample.json` | `8a5cb0bffba891f5799340933cbff38a5080285a45316b750cdf4b9bca7792e1` | 1813 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__10/benchmark.json` | `8d7974570c8bf3ec1bfe2eb4694762727cfc50c1e2bd91f246351a4efab383b4` | 197 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__10/estimates.json` | `d4b7c9b286a0de20c9624c28ef9c2c010ce75ce52674b8501633439e2a295501` | 775 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__10/sample.json` | `ef68efeaae91b2ff4f6edcebc9c038bfc6740732d43c52a316c6e8cde3dc302f` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__100/benchmark.json` | `9f464a1712b23db6d660984ab13738f8ba8de013e6ecd784b7aab32bc39aa703` | 202 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__100/estimates.json` | `52259f545fdbd8ad33bbfabdf3b1ea07be9cafda204a4151f52c5b1fc8f9c1ce` | 768 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__100/sample.json` | `7130b7941202fddcb5156c62034222a09841dfb99fd2b14e7c85f23208c0f49f` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__50/benchmark.json` | `0a375b2bd004edd8a610bbf037b832be0eb3a8d2ea6b70f63bb6a53977542712` | 197 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__50/estimates.json` | `03cbd60b0df3a53a114827d3f78e9f2f47a7f6ffe9628041453f28f1029bba63` | 760 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_batch_decode__50/sample.json` | `193d2da1cf133980781ec4bdb835095e48daaa1f8508049930c00c46eeaf0ed9` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__1/benchmark.json` | `0d0f854e4124bcd2fe3c5a6aa35a1b14404956735173d03f5e1816dfdf34d0ed` | 232 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__1/estimates.json` | `640f87da985974992855f16e7eebdb91bdd03da6e3d3b9ef2bffd346901eec0f` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__1/sample.json` | `fe794cea21e5ba81cee1f50372499256fc78d13ae56d904e29a720e9d13782c6` | 1772 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__10/benchmark.json` | `5e17e6b749f32ac9b047e7a37965948e352901e75843529397533936972b7d78` | 237 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__10/estimates.json` | `9e2a36ee3abfea9ff5aaef1df6445e237f56af474012400d0d3187235f583237` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__10/sample.json` | `c5e6b521c11bb1c59cb3a52d497bb0370962299498c7d3f67d2c1021e57e0fd1` | 1708 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__100/benchmark.json` | `72d12fd2fea8f0c7c74361a34640fcd1096d5a65bff787f22109ff19ab2bfada` | 242 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__100/estimates.json` | `e44554dce0d0e3c3b534b606a5207f376b39ccb61d4d419cc4411e8c1c5b31b1` | 771 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__100/sample.json` | `16598df176de383e5c896f9ae99bdeaae83c0c92b741c0bb54422acd9cb1ec3b` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__50/benchmark.json` | `c0a71b8109d6d1a7b88b9044d6eeddd2904d0efed70cf3bfa5269a74056d8acb` | 238 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__50/estimates.json` | `b5bfdf556b17431e48034670187a8bdfd1dbdecf44c6461946c6630b2b97b898` | 762 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_pns_16383_16215__50/sample.json` | `67be6aa2c80f29051ec0120a0837ff5df5270711b60d1889870a53d0a3a443f1` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=1/benchmark.json` | `26eed6f2d7e061e48d3b63320d407338849b21dfe2f90bb32d61835297a6d727` | 417 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=1/estimates.json` | `08f1b1d0d184ac17bbecec7543c5f39f8a3957e20ad9f356bb14ae866297bd06` | 994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=1/sample.json` | `85dbc8a8c653524d28e85b3314e3c06b97c10a13820aed48c7cdfc7079385c7a` | 2010 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=16/benchmark.json` | `a2b182c7d408c5283a8e8b1bd5617cb647fd9cffc06489c0dfd5af19adce33e1` | 422 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=16/estimates.json` | `e71eb01688e6cc6d62d9f5656c97e0096d98970906c467eb1b95278184691495` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=16/sample.json` | `e5130cc9fe78ae3cc6b8de4db6493fa421c470f6eca3ec28cb2c95cd2a29cb90` | 1963 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=256/benchmark.json` | `846b3950bc8fe1befa95213aaefb410595fe99acac049688eb08271e1e5e7f0f` | 428 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=256/estimates.json` | `e18356ba8f60a352019a7edcfc621eb6e727cca51a0e44c7d1e753e442ef10cc` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=256/sample.json` | `0064397c687da9b28f4331866e6faa3e0811177140487398fe06fba9d7e12c14` | 1845 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=4096/benchmark.json` | `d19d962c457df14eb1c839ec40d3a92a846e73b3e81b5d8ac885a3d68d54e5f3` | 433 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=4096/estimates.json` | `fb9c67d081fc0ef6b41b6860f865e6f6f633141ab3414982b4381b1de3f36aaa` | 987 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B1__B=4096/sample.json` | `1ba963c3e015594ea6ca0fa7a659b9262e7bfd07e0325460a608f354e2d29295` | 1728 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=1/benchmark.json` | `a1b8e2ae1903a8865686ac39ff370d55bbb4df6781594972266e6f79a3a6925d` | 418 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=1/estimates.json` | `1ef0ec8475c6685ca3abd697dc37d10f95290e707d55c7c6268cdb15a256e883` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=1/sample.json` | `176883995ad1585b3c2aeaf6fabd99019cc2103858b108755f9110f1862648ae` | 1935 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=16/benchmark.json` | `4d1aa5919891bebd59348ff31f028d09d83f55a6a5fb100228279185004ad22a` | 424 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=16/estimates.json` | `c3a8cce680a55982741e4e59e36d18884c4534bb9210003e8b3e24173ca0cc9d` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=16/sample.json` | `3700a2522a64921c766030db8869753f2a2233b6275f993b12b9d5be8ba2581e` | 1896 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=256/benchmark.json` | `ea59c00b76939f0eb2bf77d3333746553ad3761f8f46d54da02a95848bb6aa34` | 429 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=256/estimates.json` | `52e6cfea203f1295a470140087b813016922ec36200c5cb447e9243f4d121763` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=256/sample.json` | `708d266c335bbaf7160080ca80991ce26a691bb15da1da1a8e8930a7e12eedcd` | 1735 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=4096/benchmark.json` | `f85c81f79df807b862e86cbe21dc688869d087ccddfe4ebdabf03825429816b8` | 434 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=4096/estimates.json` | `f89d6fd9cbf0cfda5546569a2609863748e2ddc0ebae517dd50a427d244977e4` | 784 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B2__B=4096/sample.json` | `1850027fdb7b4ccfba0504352972c9b95c9b34e5bead4991b103a69e1075638e` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=1/benchmark.json` | `d3f4d467a3dcec0c02b92f5d78174674060fb6d04dc729020da243a2578ec75c` | 419 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=1/estimates.json` | `2106d377dc5b1f2a344ae0940797844950b7617f350344d0a17386688567def8` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=1/sample.json` | `83df7a0860855e904b64b45caaa82ce48fe4b6a976cfc3807cea424774636da3` | 1912 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=16/benchmark.json` | `b5f21956b0deb0290223b46d2faf6bae99bc642cb7d027f05f12cfde12e290d2` | 424 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=16/estimates.json` | `cf58785a0132f320afe8bef84f14f643540992c61fb911ef31cb660e7c7411b3` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=16/sample.json` | `ae47a91e4885ab7e053326311dfe6817f7bd77ecdced8cb6e0df7d61afb15436` | 1856 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=256/benchmark.json` | `085718e43eb38086afed714238133aea6131c4aef7c43ea1e302e582905ee831` | 429 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=256/estimates.json` | `a279ee947598bd06bb7b68d53cb976ca12896a06ba30f7787b1428c2e3a23882` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=256/sample.json` | `844cff74ad7f4d8bf5107b6e5fb5e6919a2c404f2f4031cc6a2475cf34047091` | 1733 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=4096/benchmark.json` | `61f64ddb79951769c51263fa9602986b78b7062baac740c72088f3681fd23eef` | 434 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=4096/estimates.json` | `75b4d6162a59bb5a994435ebfea8fcbb5d2959f6b7e1e90ab4ddf0dd8937199a` | 803 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__B3__B=4096/sample.json` | `6727e62621d12c596887eed5b693bc0ccb8aa88bd08ea077e474b59129064753` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=1/benchmark.json` | `a138b472809c73726c0508b48ade619031c5fddbc27f023cc2a37ab517ebfc86` | 453 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=1/estimates.json` | `5be50aeb0b4f3b394609402dac0d46837b8b60df68d766e50bc3e67be8b21166` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=1/sample.json` | `ef70f6134499244d731672d06d7e7d3f549f013e75616d95238a4a45edab3be6` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=16/benchmark.json` | `a936974f9d3f2daa563a81e9fa706f7c41323bfc3e5462e862783710e78c0380` | 459 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=16/estimates.json` | `7e0c86b1d097734328ccada8b1ec5e36d4b8d8e9351c6375e25a7a82fdb7ee88` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=16/sample.json` | `84dcb4d079c489a9c71bc5671032735641f7d31820b8af3b673e0b1a3901732a` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=256/benchmark.json` | `286e1d9119bb8f1c8273fe5b1a56ead9d86f5f65c836e969f2a1185e0d7b019b` | 464 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=256/estimates.json` | `5b796f245e80bc826bbaf11ef4699fcc6a494a429528a475fc588b08389d10a6` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=256/sample.json` | `9161f223726544b8e4e70194006d1669d73b6ce01be28244945d3b56af50098f` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=4096/benchmark.json` | `a6037bc352fc087571463670eb4aa37b69867031c70a901f50173dea385bc5ac` | 469 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=4096/estimates.json` | `c944d897c6c44ebd9534dbd4e8e9f27a683b44de7189750a6e6114b2d8e59cd2` | 766 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=4096/sample.json` | `aca31e4f33263e0c071946b1a7973cbdc9d79be8f3aa4d78170d5cf746949175` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=1/benchmark.json` | `53dbc822b3cf315e02958ccd64296ed3094958fd5817403ebdf0792fbb227137` | 453 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=1/estimates.json` | `27045d21ef55afccf52042832ec687aa738aa9426c415cce6cfd60d910785af3` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=1/sample.json` | `6302d7a3a0daa6d40d295d61fbae79cd18f0e47241c7a86f55642725ae6f6e53` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=16/benchmark.json` | `eed05bbae9f9dc986f519c3d4afab0cd6c6e3c76ae2a727eefb4ee4bec7a9a40` | 458 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=16/estimates.json` | `615b8330ae677b138b6afc21fcfa2494922a34b6fd2753b3dc9c1cf3ef63c02e` | 804 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=16/sample.json` | `f0b56d331a03f343125e2f3b47fb7ed0a1ae4158bd27e1a93af47c3a404eec23` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=256/benchmark.json` | `3c3050d035e7a030ed712c4728b93f95a2f3e32df1443151005ec3dd441877fd` | 463 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=256/estimates.json` | `ca4194b7b164fbe332a4fe536bb34f4b27e031839cca966ac111d8b57732e483` | 801 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=256/sample.json` | `900e07cd4c865ed605390edcb87c40b7ece4062b001b3cb751d79bfc9989666d` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=4096/benchmark.json` | `ec4680dd0269d51600594d9e2dd335364c038896cfe3462d031affaff6ab07fc` | 468 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=4096/estimates.json` | `72d9a4f9036a5919afdcedfb800b1ab8af0cebb61d3120649814b7872e0b95e8` | 766 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=4096/sample.json` | `d31fe11e55db9c3459c86aa04272a93a7eb0767dc48cfa3f5c43c02fa1e1a8af` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=1/benchmark.json` | `f1c00c3d82dd3e222a5ee36465461647e8b4865cca8eeb62ec8652a307d4ed31` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=1/estimates.json` | `4d370256a3e21b761c02989ac03a203d902a2c51aec00e0353e3f393e88fcf66` | 993 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=1/sample.json` | `4ed5be5a6119931fda8a1193082f4dcf88bc81479dac8dc6a3e934841b2e0588` | 1994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=16/benchmark.json` | `b99d54803e42f16425a71ea6783f0d90a5f8cc0cc8b4930a88d4203cd83f8910` | 402 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=16/estimates.json` | `e455090cc95f3c4c5919348ad7d932c3b473684a90e172ab5d039fb6a4b24ce5` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=16/sample.json` | `981120ad67dc1989cf0cb6d4f4f06e13a03b97bea4e62081e1d733301a030c07` | 1947 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=256/benchmark.json` | `10722bf2ff3e2b56a059110b10b1654458e12399093d715190600470d1be3464` | 408 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=256/estimates.json` | `297d4084356ab6e7f2943bfe0ec080d6772598cf06eec6825691600decc9510e` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=256/sample.json` | `8880de2ecafdf2e94d820138a1bbf2e5dfb904e33693666255096b3fd5ad0426` | 1837 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=4096/benchmark.json` | `26ce303ba84c7413f04c6524ca313f1d6ca766e2389ca3b218293bb03fdac7db` | 413 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=4096/estimates.json` | `888db11fa20e8e686a272da75511fabb23cc2d85c881b35124eb7e3ea55a5f58` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B1__B=4096/sample.json` | `5fcf30cfd4cde3b7fe3787c8201144f3386787f31ea1b5c01965c0df40bb9a53` | 1733 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=1/benchmark.json` | `dccea5e7a04d3908d069d9ae8fc9e965e4df0dc5a257411939d2e45eb9d230f3` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=1/estimates.json` | `3a09b4d1a2a65b22d70cf6c3251bf8f6790983a41a3806aac7c2c78d886b4edc` | 993 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=1/sample.json` | `9be349a0682879aa14ce7c065f805915540613c64b4b5e08f86e051d61a770e1` | 1922 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=16/benchmark.json` | `0e8f371d46624d7b78fb5a311ad342c9fe39f213896e6e7742f9cdac4c3ffe4e` | 404 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=16/estimates.json` | `2929463cfa5d17f5bb6bf0a11ac191dc90122c0b9981bab203b733af1ffce005` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=16/sample.json` | `3e001e6e3c6558047f044d493e98d52114a1c6195ed0a57585657736eea5af31` | 1894 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=256/benchmark.json` | `6033527614ac7177e375d9a385f23bdafc24f8d801274cc6ebaa1f20d5d81fcb` | 409 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=256/estimates.json` | `7ebb8eb00cf562e122d5171850b0850cc8758786c953cf4e9f7085807e242d1e` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=256/sample.json` | `86274500f8c95936c80aa1c028cf2cebf46d662f84b962ae704c1fe983faf55e` | 1729 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=4096/benchmark.json` | `d4dcf751ccc1c2deca2f9b54d135a514adaf53cf9932589ddf7b0cbacab39432` | 414 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=4096/estimates.json` | `c5fbe9fa14155d18ef2caa16744bebaafba8debb78876b655e327ea223cc15dc` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B2__B=4096/sample.json` | `0258d5817ce899c386be1631acccad992e6db4c39aa2f966eb84fee1a6295b44` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=1/benchmark.json` | `634f6020e20dc4a73b0e414f5e612513cd7d962d1479401e1de72c496fde4415` | 399 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=1/estimates.json` | `2862aac0730c36cfcd048b0a707279199987f61daaf5d68ef4a7e39a26597538` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=1/sample.json` | `eccb79a2d4d72bb813041991f42a3105d42f7c1661b7b99b4b920f1e6541d1dd` | 1909 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=16/benchmark.json` | `f8edd318fa42e58c3d6cc0cbbe6c577a5334fd72b50b9bf25d1215e3669c0c76` | 404 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=16/estimates.json` | `1c4198c9399c7a90ff22058cfece22732dc9042bde787c47b77cd03a7c2992d5` | 977 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=16/sample.json` | `cce0808fafccfe4d777cbe03ea6c921d7f1c5e1ed1479f435ca87421ed4fef4a` | 1851 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=256/benchmark.json` | `a826de8246e8532da2dc40c60d89018bebed65bf0128d4b1ef394501c8301405` | 409 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=256/estimates.json` | `fcee6fda68fe2049630cf78686350951a91d988f29bc266d1a3d4fce0faa66a3` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=256/sample.json` | `dcb25b8a902bd74695d6a891254405411836dd3f9b552c065cd45311a7884e41` | 1735 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=4096/benchmark.json` | `5df33369f59ccdfe44c099e2dc14ea0fd653384689f31078c144effe3e60b52b` | 414 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=4096/estimates.json` | `96e39cdf86b7506855b0944bc305a81e51b1bd774401655bc0d342d5d922fc18` | 795 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__B3__B=4096/sample.json` | `c6a7d3e81e620129cbd0b840b17f6801dc107d62e03a89218e2eb978bda92b6f` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=1/benchmark.json` | `0b23085e8c8512fcc2110c82830e4f56fd83f420988c87d1cc7ad1a52eb5c895` | 433 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=1/estimates.json` | `89ac9e5b758258deba8eb359ebaa818460ad14096d31d4762afd36253e55f8d3` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=1/sample.json` | `1f769de0d704f8bb194d057458604a303eced3c97bdcf964bd3994c6783b8772` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=16/benchmark.json` | `fae0b7073bf62c6c397d72f3631b9d3b775d59fdf7f14541e99cbe3d2c87c2cd` | 439 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=16/estimates.json` | `b85bb3651fa1fd793695ea1c1f9b88d5e116f2bcd636e2198ff96f3cc9a2b6f7` | 781 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=16/sample.json` | `b2c2bdf3f35f04c917ead59fed84eaf573fc865466c740c15d40de21ebaf10b3` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=256/benchmark.json` | `272c818ada54638362043d9e750bafcd41a0a3c4781ac35d18b952f084daadb2` | 444 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=256/estimates.json` | `57ff29417278224dde6e09fe95a9d15dcfec773c8a789a305c7a43fdbd20da60` | 771 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=256/sample.json` | `07a9d97e16bfe9f7a22a4934989276c0deb840605f44298c6b2732a7034e69c8` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=4096/benchmark.json` | `a01501f459332bb1ca1625cbc3e75293091b65e6500063ce07c9922ce359f836` | 449 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=4096/estimates.json` | `0e4469015208d320412243f3008e95c9b9ba538cc95c5cb608c9e0e930586572` | 770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2N-mother__B=4096/sample.json` | `be7a80486565ffb420a2092061e86c0c2c8177d8efd912573e50c87ad5e9df1e` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=1/benchmark.json` | `e04e8d47dc50e594acc33db3cba60487c2de7bd5756eff833c7e030af3ebabe3` | 433 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=1/estimates.json` | `aa8c38d5a584e856e5759fea185f508f9b6c98a3065a8e56c699919b91a17860` | 799 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=1/sample.json` | `1aa97db6b716a26413dd42adfc117404035792ef8bc39cf21c9cc70ddc50f958` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=16/benchmark.json` | `00116f205b868130cf11d2a533b9c07f999f380633d72c98efbbb3d636ba4de5` | 438 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=16/estimates.json` | `b74ef6255316bcd7b1d50ec01a50074d46c621164fed6ac6a1ada4b83df3d6e2` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=16/sample.json` | `4e1f746ccfcef389a9951208cbf1ed6f8dabbd58e869871bae212cfa6d584d7b` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=256/benchmark.json` | `f45bf0f294aa4a5cc0300735e24d9ad2393bfd9f7fc3f0ad01b5106d8681724d` | 443 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=256/estimates.json` | `5c62a62858390975705d39bfa1ad14dcf401e5416581496d50738a1c7e45c8a0` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=256/sample.json` | `7141612a44d10f1f59f0620192bbef7494a62e1c0938972a86dcd87a297f84fc` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=4096/benchmark.json` | `2f91d39b4793063aa2731282feca2d5b3b1a02d84f8f09d66402f88e714201f6` | 448 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=4096/estimates.json` | `ed46d60e8c39b912e4673610968eeadcba3712c3dcfb984795b68d4ac2a9d39a` | 765 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=bitslice-interleaved[scalar]__W1__warm-reuse__T2S-mother__B=4096/sample.json` | `1911f46bf53e89ec6fdbc379fb10efea24bbbc135b3490c99f00313aabc239f1` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=1/benchmark.json` | `5c9cc156b1063b6d101739c82408662eedc4f85ff162b23907abc2e02170dadf` | 377 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=1/estimates.json` | `b607bea7def3b6cd9f630ee32c1ea727b4455409b956a509086abba36074b9a5` | 995 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=1/sample.json` | `fe88248be511091ffd472874d6f74db02db9727c8e638ebef18377dc2bef417a` | 2072 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=16/benchmark.json` | `4bc0911ee0e24ccaa50e75aa76304864f2d36a43f38fae44669f60865a469601` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=16/estimates.json` | `477197c7ab568ad614dc8fbb99f4114bc643f00ef5efdd6e4d22d0bccbb76282` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=16/sample.json` | `770cefa034d62dcefc8c42b67972c628817df7f532088cec3e356b3bfc5d0223` | 1967 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=256/benchmark.json` | `0505bfed7f50a7564359cc5ca05710cc3efdf5eb4f4c76885386db726767e978` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=256/estimates.json` | `4a3833b58c562be69fb1ff99825d7e298157b974e9fda3ff24517b2e2558e050` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=256/sample.json` | `7592b3541966c8c6d9223585bb3f182d5a3ad70e84b94473a978b1b2489bc101` | 1829 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=4096/benchmark.json` | `b9f77346e12352f25f83bfcbb770801d2a13709c2a90b62dee874b6974ece840` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=4096/estimates.json` | `2338c26711fa4f2de3902366d00fe4df40241614d3deaf21c387e04698cef17d` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B1__B=4096/sample.json` | `9c0f0e2fa980168d6a7dab4dca3ea515f4bb1b0f1c5f3c112c1537ea47882cd1` | 1719 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=1/benchmark.json` | `896ccf9aa45d5fd9c68eeed6f8f1e33bcae7ae8685f885cd24f8c5c5a94835d5` | 378 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=1/estimates.json` | `6fcfa4f88b9b29b1f0dc8003010b4456b7eda0649ed2996bb0386df3cad6a240` | 994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=1/sample.json` | `22fc55d16de3127d3bcb7f1f026aa6a9164dc8d1a080a98cabb71f995f091b58` | 2008 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=16/benchmark.json` | `e33400a2a0fee3d5049bad947ab5df8a5712e21b119d6723d256bac81f6bc836` | 384 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=16/estimates.json` | `ca316e67c4dcd546bf37167436b91820578fa987bb774121db8e28341e8e17a8` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=16/sample.json` | `f18672ffbfc93052cf21c4a0f171e2391a5814aa5886941175babe557742b66e` | 1904 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=256/benchmark.json` | `fbb36c4f7fd76ac3318d90e2df0b57fb3c6616ae675ab750aa15204062f4a880` | 389 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=256/estimates.json` | `078fd7934879c802232ac2c6299fc663cd4ea603415460b4c069e99c6649edef` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=256/sample.json` | `f1e6791e03818fce47b7919ee3927444aeea0c49b1b37cf4960cdadb14e2b6e4` | 1741 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=4096/benchmark.json` | `9ab7a628c5615aff3227c4ba9bf5e33e04e803737c3421e3409a991033c9efa6` | 394 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=4096/estimates.json` | `78e24a22b90fcaef68a2f96548f959a9f7c4bb514ef7768708bce69bad93a908` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B2__B=4096/sample.json` | `d9fc2c42454a7715eff2b8030dad679026ad2bb6735e70dbeb05d276ba88c1d9` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=1/benchmark.json` | `581a709724f1366814fb10987b8e94612d40f6a9ffcf9dcd0abe8eff173a2891` | 379 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=1/estimates.json` | `5d7443711b1f669d214ef61d00f744b09c218404be10f5481af006f27d8bd151` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=1/sample.json` | `87a9fb1293addd5daee0e57c68f6fd71450b29179f253865a81be357ff8eed73` | 1991 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=16/benchmark.json` | `894a6c7596c133a1ececfb667ca9280e03528130e0fb907cd5f2b9cb779a9ac7` | 384 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=16/estimates.json` | `c7d945b0a5e829e6a6dbd0063868d9d715dc7436f91c8df26103558aa15bd9e4` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=16/sample.json` | `ddb2d3389909431afafaa3b10b80e446ab23b42acec6e0f89e70000d472d1f01` | 1874 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=256/benchmark.json` | `b2ec1d0d16b356c27563d61bfab2e73236d416fba839a52682031866dd76b970` | 389 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=256/estimates.json` | `bf4125748c3cd15bd12e82bd8046a8ffb116297e5f694e48386f7197d7dafe5c` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=256/sample.json` | `4e8089a7e5144cdc2149c8b99eb1ed493672ff0220939c02f5d179aa103478c8` | 1720 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=4096/benchmark.json` | `73297c39f49f367db7be7001966c575e133c0f10947f215019a728440a85d613` | 394 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=4096/estimates.json` | `c2b029d012286559d3c951c38ec8341160519448dba9db9c5889a13f05d827df` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__B3__B=4096/sample.json` | `3987c6696300370204ccf1ab08a2ff87abc5fe593be998cb76b06d51d02b252a` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=1/benchmark.json` | `72da2840f7390910e95b119bc8f9f9606531669553555e09446544985c1459a3` | 413 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=1/estimates.json` | `31d52b1484d95a52271e7e2d5fd61294a2ba36492e472dd0c9a5d7a8d4e8abcf` | 804 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=1/sample.json` | `56febc417f100103be6c74c8ab73f64b3df8e1954c3e859c55496d10210a7eb3` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=16/benchmark.json` | `89bcd6dc4e37b1fc927cf8a10af35c06d329559939fe5512f657229e3d3af64f` | 419 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=16/estimates.json` | `2f1199910c3b84e81340eea3f1b3d558efb00d970c69239c8efc3fd484331b43` | 793 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=16/sample.json` | `f17154a622c5cc52c7a5486e3afbd8c80f3b874b7860a19ccb2558390deab707` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=256/benchmark.json` | `e60a47a297010742769243c4685c71a7c001eba6161ddba124f0179063fb36ad` | 424 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=256/estimates.json` | `011d064112c0338114aae4992f0c820ce4051772cd60800ded11481ba4749e2f` | 781 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=256/sample.json` | `5a174aa99cde72a7269492ecf23777bfd10f001c8d652d62b1f79184fcd09391` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=4096/benchmark.json` | `54c9ed3b6308d7debc3329b627374ffe4e2a0f8967e031077f51dc72fbc1ab19` | 429 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=4096/estimates.json` | `5e5427967f1b7e4a4a1f951aeac51889a676ae0eca9be39d8b5e36bcc07f38fe` | 762 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2N-mother__B=4096/sample.json` | `ccbb2b13b35c4819431df3431099bbcb13b3c68a74e90103937120df422aeaae` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=1/benchmark.json` | `155fa20217d1514781723f97723fa1643a1af96700f7122a1240cf6cd1a70e04` | 413 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=1/estimates.json` | `7a6f8d8548924f1f08784ed349d86c8a3c9332a9d7f29a19c69df11b5800f1f1` | 804 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=1/sample.json` | `6a4c3a1e5ba6f60ed7194b249d3c7a26ae490466b17d47e421faa720412beeb9` | 244 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=16/benchmark.json` | `9704c9addb7ea1eb6472dd6da66b3bcdbfd2d668e61c7173f1e82331358e9311` | 418 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=16/estimates.json` | `506da0deeb8aeb3577ae8a8e5b32c3921b94871bd880cf44eff7b00038c94ddc` | 798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=16/sample.json` | `6e4397f7a7ff63ee0ed6c281781a5adfdc447f1456884da336539ad8eccc54ef` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=256/benchmark.json` | `85afb9bb69e36b274a8fdfe463564b58e0c1ac469fdf8edc3f1daebec0a22156` | 423 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=256/estimates.json` | `52901bec4ec6b827518708eecd10d79b2b1723c4d79b95f6069915c16fc0f475` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=256/sample.json` | `3ece3a2958764fd3ea5f321975fdae434bed9242f16e08b8d10db8da793c61f0` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=4096/benchmark.json` | `c407b3d0060eeb742efa60b1d26db6f282025f72dee2a0c3efc785a3b07123d3` | 428 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=4096/estimates.json` | `716aa12dcb49afdc556077eaecd0330c2eb65ffd6cccaaf90caa217a20c80220` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[avx2-pclmul]__W1__warm-reuse__T2S-mother__B=4096/sample.json` | `68f279cf87144de32b3eb4b144954249407e37238405d8e448807f48803fd7bf` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=1/benchmark.json` | `6d9dd5f51b79c3ea641291505cf99266abdf38001747e2d06dac1f12da16221e` | 357 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=1/estimates.json` | `9699ac79e3c03ae23c7fffcfd983b74a9f0f191db03c7ce5626ad7288b2b376f` | 986 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=1/sample.json` | `668bb6f851be1de66bd683a4e58c9c419b128164b31c75b5f7e1e82b81bfc553` | 2045 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=16/benchmark.json` | `8021177f04f1fea2935061836e722d979ff7ad6ec2d865cc37e40ce873334b47` | 362 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=16/estimates.json` | `3c2815129bd42e9d7a7d618c3132aee17bd3423338cb9f205acbde7ff4383a95` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=16/sample.json` | `a805095fee1282247c846ba7c3757620365603f0e5dba53f86031142a01c890a` | 1925 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=256/benchmark.json` | `881fcf2d422f2603b819b6918546b403386375ca5632640fea17acebf811fb73` | 368 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=256/estimates.json` | `7ba08815be99671ad4a542a61a0c0833b35c44915e1193718c4aec30d43e3fba` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=256/sample.json` | `9bd1f6b22ecb9dfabce638959009b1dfa4c95e514d9e271778029a862ea41b9f` | 1820 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=4096/benchmark.json` | `b6bf38868af924122aef896dd8d68cbff3f50c7c91ee061ba3891d81cdb0f957` | 373 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=4096/estimates.json` | `d9832be5c54abc4c2fb450132d95fc999f0fb3f4b904613090b2981774432270` | 968 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B1__B=4096/sample.json` | `ef1bb2642173e1134acf5955c3cdb9ab766167d577ae1c5c1884d1d1ca693641` | 1731 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=1/benchmark.json` | `3c13648c4b735e8bffb5a4b79bcdb61879559ec3d82e81b7c8be372e51c93c89` | 358 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=1/estimates.json` | `551f6132e7a4ad0289dcd0590c6469af2b10322d42704bf7ca8c925ea0f75704` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=1/sample.json` | `77d95fac501c2b5d6022589850a7051947a1430c4956e84c4b2a24b35c1c6a1c` | 2001 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=16/benchmark.json` | `9ecaf227cfcae0c7e0872882022ace776126266289f89e644f38f4662366f886` | 364 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=16/estimates.json` | `d516c2d3d7749fef2a6532b1e8667a7a2ae13059be70ab87b7fce1fc8a3191c0` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=16/sample.json` | `fa460956e4db3e2d8846478ff830472b53e23bb7b1bee954fa39c68c9063c01f` | 1893 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=256/benchmark.json` | `75b79a5de030ecd25ad9fdd0e9c9f28cafc0e2775399acd2c3b846fa536eaa5e` | 369 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=256/estimates.json` | `a604cdb46a548a82600ec18f75b5639845227bd245e97c1ea94fa6aeabb2c7d3` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=256/sample.json` | `fa0194d6adc5ade60c10d011927cd864f3a6637066e2e49df807d4507ebc398e` | 1741 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=4096/benchmark.json` | `4151a529aad535e244916ed7285c98bb3ecddccd03cd459ebcaf6cf0f0122d3b` | 374 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=4096/estimates.json` | `15269793a59be25376ff2d1b96f165c32995554e352cd5be065ab5bb1fecd4b6` | 798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B2__B=4096/sample.json` | `c19c5a9d94295c4cfe6ff166afd53d1d5b985465c52ae7138ed996ab4c362063` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=1/benchmark.json` | `379816a6bb99c5b3c03011588ccd60eaf8953f3c5895dcd59540c703447d5d18` | 359 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=1/estimates.json` | `d70c187adc72a3d54883f4a43a07c3ef76a51b21335bfa5d92ebc7bf62e23b98` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=1/sample.json` | `fdec4edc52ae1dbf384649d64db7e03935b44c12a12aef52085b58614ca17f85` | 1968 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=16/benchmark.json` | `43b4d45c9fc2513d97e6fcd167e089f605525042556d0be7fc2de5c2ad15ebde` | 364 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=16/estimates.json` | `7ceb2b77d9e56f1fb88a2875550cb0433b2ebacb82a8440399e3083658e6a396` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=16/sample.json` | `445c1202574d476fa22494366d5b86325c5d9491cc8c2c5d92e823416817edba` | 1834 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=256/benchmark.json` | `541fca7e7e588a6db071d3a4548ec9a34c5f80a2f016ea21a42eceec95b49ecd` | 369 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=256/estimates.json` | `a067a832fba39e40ded7b4658aec24aad55657b43a3b005a4e0fdc058466474a` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=256/sample.json` | `88e6352e1eca569ef5d5b5a24784ae761b5d18a306314d4da5c8e734bf498cbe` | 1723 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=4096/benchmark.json` | `dec39dad08e035e3438f8f8b296ef9f8c1dac2fe73d15dd7fcfc9a6754b4c625` | 374 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=4096/estimates.json` | `c54f7f006cec5c389e5a7a61dadb3fe82a61ca23240e20ff5020239d0f04051f` | 787 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__B3__B=4096/sample.json` | `581827f8c93bf429cccef48e5c22dd4737da9128d14fa25d9dbf629fa71d85f2` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=1/benchmark.json` | `bb9cf5efdaa45cff9ea30de071c4a5a952da9b3a193913d6f0e2025387e7a1e3` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=1/estimates.json` | `6ac07e959bcb22e020c998cab990788d72149d8b2a79d50eaa1e1c9063151019` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=1/sample.json` | `9940ad84406b8dbc5d6274823591a09a26c5d23e9985efb1de62573f43b96130` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=16/benchmark.json` | `96da02883aa8f334c8d4ed1e98d0006874c9ef9cd181e3399f4ddc4f4320e06e` | 399 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=16/estimates.json` | `e0ce155003a6abce922483330cbab0b57cc557baeae72eb67c76f4b3f4f09330` | 795 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=16/sample.json` | `bfb5ccf0c92b957c81bf93479e26b80082a48e1b932a22db760cf30f811ed887` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=256/benchmark.json` | `47f3c9b3553fce7f96f5970833b4ef9f6f40d435d0a00b688455679615c3d81d` | 404 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=256/estimates.json` | `b806b526d53866f0ace9b162ac9fbeb5b8d1c31d6725fd299f01c8c3e51221f9` | 792 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=256/sample.json` | `6a6adf3dfcf21338225aa1fd5924a0ed6f5b29d9848a3b81f3bfa9236690997a` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=4096/benchmark.json` | `5725b58a7a1dc2e0ed5d7bf06f78bda0029f75a95e0f221e537b8e04ce961f98` | 409 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=4096/estimates.json` | `4e12439bdfbc0d6390b5cad6d708b083bfe393245825603a7c2d39746921fe85` | 770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2N-mother__B=4096/sample.json` | `13fa6c7b15019cfd9b0861c6a4c6d4428694a187d4221314875ec116096549aa` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=1/benchmark.json` | `0203117e3f92f6071759bf597fcddf69ed67a83403296b1c09b103fbcd226559` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=1/estimates.json` | `00d7bee24fac39c209de31676b2bbe24b9e59bf7eefce8476f8449b99c1faab5` | 794 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=1/sample.json` | `3384593dc394eab20b18d2123a3f101025c3fb7422b292d4996f22e1cba2c583` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=16/benchmark.json` | `df67715bad156f2a1e57b01768b07e5359d9bfb7f09c50fee2380d4923d942d1` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=16/estimates.json` | `ad98375516b625be34b51a208b229caa9da8349d68eb2387d91f0cd9b299c6fb` | 801 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=16/sample.json` | `6ea41943dc64cdcd1e1b948d29ac44c3639ed01739997ff027b152bcf64d1494` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=256/benchmark.json` | `b7485c6cfb82c50f84fc3698623da2326b6dec161251d773ef2422c54ba8817a` | 403 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=256/estimates.json` | `0435391579760cf6676a8af2da754911fb6400c273cd3b24ff154b5e2c0ecd23` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=256/sample.json` | `cf39bb1135f6a3c087df5fe5ffe98e74f2045c07426ddfe01d1ae64def930f8c` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=4096/benchmark.json` | `e1a227fe93ec3350c0e643ad451eeeb9c1d60e8768ae7316a75d9bfd2865d702` | 408 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=4096/estimates.json` | `5470c7c84dccaf40f3cb10ca278c49c0018ac9450632357eda98d58c135a1ea2` | 770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=clmul-fold[scalar]__W1__warm-reuse__T2S-mother__B=4096/sample.json` | `6c5cabd18fe4371085a66194e2ff14e78e0bb668481c060b9426b18c181624af` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=1/benchmark.json` | `49e4052476095812301f6e8a3054cc65ccabd02c30cd3762742bcec02c17284c` | 369 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=1/estimates.json` | `d9de2502539284bca5053e217a70f28dfa2470484a7b41637bd9096ea4c0feb9` | 991 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=1/sample.json` | `04028848ad27efe5bad745125161c7e30824f996ba8a5b383479b2b30ecb58ea` | 2062 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=16/benchmark.json` | `b7b9c9b4c287c7e09719aec72f425d41af28cdc59b35e9b7a53b5f2f8da5fe59` | 374 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=16/estimates.json` | `166fa6e929d4b248d799a8f7d49cd6125b42a938c65f8fe5952ff54ec3250b35` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=16/sample.json` | `18041d1d806a124472165fb0bd2ac8c8a357db1e1d57aed73bae76533d9478ee` | 1965 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=256/benchmark.json` | `0d4e9d51cd3c1abba30b20692f2c08f7eae1f952d10d84511f3c69c299344b96` | 380 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=256/estimates.json` | `4088bb3b36867f56923bbbd47680933a5be76614bef7344635d86e6bc7337d5f` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=256/sample.json` | `c609a133a3d09b89b4350a1e218d63d9b5cda2df4046e00852db04fe50026a3a` | 1828 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=4096/benchmark.json` | `0bb0330e548a4af6e6b095690edc5d1bd153b71475c3266bce4b7645928c0aa4` | 385 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=4096/estimates.json` | `6b1d55025ca2f91939ba848014c5cfa33209eb42e7143a54ee60b4da005f42c1` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B1__B=4096/sample.json` | `8f80d66eb15aac092606e3e231dff20dd06fc90a8f732ef52f1cbecbd7d65d29` | 1736 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=1/benchmark.json` | `cf220898c89142c55359b5f8e201523bf5facfc1d93e6cd72f58e3f760459d75` | 370 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=1/estimates.json` | `ccf618cc660cdd4b251da158b8882b7933feb62dda3bb67a6a12da0a00bb76d9` | 989 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=1/sample.json` | `a9785889e92782bfff7fb04a39f5fbd9f368918656ce57ce6930d554daaf2622` | 1994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=16/benchmark.json` | `1a333136e814f1ffbcdb648450661532485f90e836071cf6974ec2dc33be394f` | 376 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=16/estimates.json` | `cbe77ce0fd048eb845c355bab65e5dc4d18224ae57f4347526def0ce450f3a9a` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=16/sample.json` | `531b43af3a1f2f6652ae0db2f86e4b7f9a03deb49ff5bd1f82112690ec83b508` | 1884 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=256/benchmark.json` | `55364edfa00e6307fa19beb3e5b4f16c37de938922c90f425e4ffe26ca707ff7` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=256/estimates.json` | `7c5e719b66d5ee17f9c8152dd7b7e8b9bc8208706fa12f2da3ee1088204c9a14` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=256/sample.json` | `fb1a187601c087a458aa5867f12a8bc6feadd78a23cff2f9e5302b67bf19a8e4` | 1743 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=4096/benchmark.json` | `ced1f74d7da9a818df0cc7b691b04438429cedcdd235591443dbedb42620f497` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=4096/estimates.json` | `0b8e3d0f961f499706b6ca7935f86680f0526695f434c7d8eacacd2a55a27398` | 774 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B2__B=4096/sample.json` | `99ae49924f525a7363d169ec26ad11e69a19d32939f912bf50744add44f664ae` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=1/benchmark.json` | `de4c6e2c46abee8f323ae4e14b430b8f41900aeafa76b024927b553bdf2edaa8` | 371 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=1/estimates.json` | `4bc6978fcb509fec2f8c06520d8cd140fc18cdf6a805073f051c19c7f7eb7ddb` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=1/sample.json` | `9ba9e8ab2a417527ef29e3fc35e2e4d2aab5ce462ccdfd141e0cae1758a25e5f` | 1944 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=16/benchmark.json` | `77acf8a59fc06ddd2c4728e6aa0f7942a28222cd87be15e079106a296f9ea55f` | 376 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=16/estimates.json` | `974f1dd520ee0194a5bab3396357b2ab782de2de20ba51dba0f7b5e6524b4fdf` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=16/sample.json` | `c8bd70780d35bfa4a6c9b24f19fcfc3ff81b890e48b68e2745a5b4c98aef9335` | 1822 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=256/benchmark.json` | `c3a7ba5c647ef5e528506217a914db9ead1d43aa28e5ccb425486c94a76131da` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=256/estimates.json` | `3f3c18cd9577ed81ad7e1ffb6eca2d4db00118e64522d3ae4165b3dbde747c31` | 970 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=256/sample.json` | `d4450df06022be7f8577b2147d3cff1fee0a66db5d7c4122181697d4405e947c` | 1728 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=4096/benchmark.json` | `d7c4b9b1e0e20499e00cfa60e18bf86980b4e75b869860c9b7deff0a50c37489` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=4096/estimates.json` | `2160ebfc677690181415b9d93e1c7ffdf5998069494ea8e453dd9598515f7492` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__B3__B=4096/sample.json` | `bb6cc0fafe0c8f116642a91ee09e21bbfea74ad65870a3b85882559d5b7d0059` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=1/benchmark.json` | `3842e42418eaf02049a228b5768bcfe8511906b9e1eeb1eea44c1575f084a988` | 369 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=1/estimates.json` | `0a1d3032aacfd583b86cded3a761299ab63d494d5e8470d40b334ce2b0df084c` | 1001 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=1/sample.json` | `7a2e13830b9daaae606164d78c7b0e7b811dd18b85c6f40abd5cde8399eda606` | 2024 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=16/benchmark.json` | `9a9b59e3a48b2d78007974d25df929970a86c4623f6948393548b077d9f3335c` | 374 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=16/estimates.json` | `7b7ebb02f7d0155dea383167aae76123c6777a24138481642b60fcbe823c0f36` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=16/sample.json` | `03edcf0fe07eb0161b6624d98a84e6934ad87ad5f835bff1d73f1c9833c336a6` | 1922 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=256/benchmark.json` | `427395927e05894ae393cd532e052e16bbf03d370889d876275f96c98f075b7d` | 380 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=256/estimates.json` | `f5b3b1a3c2758b4cfc67cfa307b0509133eb9549bcd81675a2a01d3d60656a4f` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=256/sample.json` | `c5e945154ed98f61aa1bd2991fc68e851db0cf46e082d144b445b7772dcfb13a` | 1816 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=4096/benchmark.json` | `e11543d35ad3d95c1a34c52e90a770597ec2bce4ba3076638b93f971234ef7a2` | 385 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=4096/estimates.json` | `9b1c88459db1dbd165707dd96b6b8104e958db888702536fffa00484cafc3e22` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N1__B=4096/sample.json` | `c86bbb4d2161166e463fa66421dcc9e611a6ab6d68af0d0c8152c277240bf06c` | 1737 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=1/benchmark.json` | `f298e8b1a1f59a368272f77d7d98344a05a085a56e688b02a61f152220e0c890` | 370 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=1/estimates.json` | `d57e89585241f28794c77121d247bd4a94d1f89ed92c599722c95d58289dd71f` | 986 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=1/sample.json` | `1859200012eb30b6d44f36fdb0bc469a1a753d2d6996e704ff104093c13d984b` | 1996 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=16/benchmark.json` | `92437bfc8127944ab99528b7762bd8d7b7be3c337b4a640b8efbd02718a5aa2c` | 375 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=16/estimates.json` | `cbdb913e3fd77e11413cef0a39fd33bcc19d6a75f881682545d5dcf8b26ca838` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=16/sample.json` | `4951a2fa985e32e5416777b6d9a49160fec166cdfea08a3311146da42644d1cc` | 1875 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=256/benchmark.json` | `20151cd7009ec3546de6027c5db887ded3fb8a3d8e6533f6182fe16902c74034` | 380 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=256/estimates.json` | `6aba3459f62005462ed38d695cdda9de8d72cb2648b9f31d0bd105c8c160a6a4` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=256/sample.json` | `252426f5694275cae0eaf8b82553db01fae3d7739768deba84d361bfc2f93d01` | 1752 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=4096/benchmark.json` | `e2ec421431a0e67c9e257a85c170755145262b7b2f19e7bc255f62c2c82ff7a0` | 385 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=4096/estimates.json` | `9ab6da8d9412a237de4f1b7fee40a72089c3defb23febdc3ad4a33f7f21973db` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N2__B=4096/sample.json` | `3845fd5f42248cd914f1f49fd1f0b6d2f25e0e95d30c71ab563a26fbaa6f0aaa` | 1660 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=1/benchmark.json` | `83087f5c9bed62bc1a061aa2c0cf54a0eadbcbba55999d98571b9ad75b513794` | 369 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=1/estimates.json` | `4912a3207cf959fd511c46d37f8bb09238a5f7cd4fcb553c26c618089d3baede` | 989 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=1/sample.json` | `49decde864fb7407cd89f6f68d2956f4ba393057bc4ae1430a2f4ef00ec347ec` | 1919 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=16/benchmark.json` | `cc70ba2eb6d610548f21fbb4136ca666b2eb26e2ca93c1390c91edfc7a55e98b` | 374 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=16/estimates.json` | `e1bc969da0e45ef53e55f403e4b28ba7222ba84046003b46a9c52d7d362ee54d` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=16/sample.json` | `4ec08b2b17191121c76a2d4e20c12d66dc3dcf00d154fb494ffd7ed6a9e84723` | 1813 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=256/benchmark.json` | `162e93434f545dbf86c4d9c5a3984b14486564cf48bf353a7784bc432d7a3989` | 380 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=256/estimates.json` | `e72551220f78eac66e466b84bd752f12f98d436a7f75f26deb652a61850e262f` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=256/sample.json` | `4c9b2a0e06541025c1bf16b19e608d6bf0cb9579ca6041e72c96d9285ec34c95` | 1729 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=4096/benchmark.json` | `0831717fb240472682a778202ee96a3e3e1ad80292289f3a8e6b79921688d416` | 385 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=4096/estimates.json` | `67c44ca66847d3e60f2159086d71ac1bbe443bfe246b75e343af58ada882a1f2` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N3__B=4096/sample.json` | `2909e80ff0a23f710ff6cf7e892cf99a4098e3e0cbda585ccccf421ed8d8d56e` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=1/benchmark.json` | `494e0804c6078360f95a1d0a3bb9d12f536de765ff2041301ae1cdffc23709f0` | 371 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=1/estimates.json` | `20895fb516ada9d9ea5c6d62bf68ca73d9de95aec9d874a8bc59094c0de02ea1` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=1/sample.json` | `0068c739646dc99a781910a5599cd09474eb6d6a0c15f4c3b6b6e6376debe712` | 1752 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=16/benchmark.json` | `f8cf05b8034f9c12f2e00f99a26723cb549761a763bf72eeef0b8b55317787ca` | 376 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=16/estimates.json` | `fc0cc478daf845c85b64be6785852b6228bb4370decddc6a2c62343e392985ad` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=16/sample.json` | `df11b1ec83d1176db690b6172a368167018435223f3962b4882c29ada3794212` | 1661 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=256/benchmark.json` | `bf39723b9c2e157c73076b060cfd51a8d4069731d3143d133c7e48b0fcb5c5df` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=256/estimates.json` | `84f833129445ea2815485916247fa8f93de3a4590bee802809fce88f9dec3ea0` | 795 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=256/sample.json` | `86aa705975184037e202bf057c64ea89fc4259eaba03ae7bd2d17280355efb75` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=4096/benchmark.json` | `a3fde91ebadba9a5fee081d760a43fe677fc82a49a674df860563b289f86afed` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=4096/estimates.json` | `dc6cb86d66694348f4f727a06de22dd7b5753eb34f77d93748239795c7f4c44d` | 775 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__N4__B=4096/sample.json` | `ac2d91ba08720361d6611e831f3f84f83074bc079b6a460c4ede33b4165bc955` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=1/benchmark.json` | `540969575720877d71f715fc941b6f0f54764e03fd91c4b96a0c42478348c2df` | 405 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=1/estimates.json` | `f017ad1de5986e5d4406ff5ff62a3e59986c7b8833fb9bee6e353cdcd814ce99` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=1/sample.json` | `15fae2952ca3f4b6fcf4d20b9ae512cba5785244b94e9bd68fcae0473a84d112` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=16/benchmark.json` | `d4f0f1f86db26e2c932bae3c5b37e0fda736e346ef3b00f7703b2d506d9d5239` | 411 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=16/estimates.json` | `42e597f2b1178adc17f63ef8a637c2ecabee9af190761b29d3f9250fd277fa33` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=16/sample.json` | `1ea285284b0edd82aebf626ed4a5c5e65c841888da3e2d589d017b55459db44b` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=256/benchmark.json` | `49c0b29244c147cce251ff441ec59023254d2fe1f54ade506098a42cf6103431` | 416 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=256/estimates.json` | `4030310fa53049a62e2422bd813385316cc402efa9b78ce211c3f4bcfea62e88` | 769 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=256/sample.json` | `6839cdc337f2ee8f11ec607e336facb03af25261e55cb683dd0c250eacb3a3c5` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=4096/benchmark.json` | `3698275c1c504a5d382113f3092b5dfd8b918b8b537a22d00d91a52000e21161` | 421 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=4096/estimates.json` | `3ee0abc6dc29affdaec13cf981b0f929185502e0975072b5caa12d7a41c65764` | 771 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2N-mother__B=4096/sample.json` | `bb2948f205e421d513e21d69e83fcbd55a51d126f913fd9699e635bc317a6ca0` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=1/benchmark.json` | `76db5365303368470e2f9bb7e1da8663d3f76c4f309ab20ce28387f3186a4ad6` | 405 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=1/estimates.json` | `14463d55ae0739d05f0dcfd753bb1b80c18e887d795ea3cd6dfa11d15c4cc230` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=1/sample.json` | `b21ce8d7921f1f88bfae5304813bfc6688e42e006bd1179e919ae6de6ce0707e` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=16/benchmark.json` | `52f7608e492191318298f8b091a2baa689d3fb46adb48b65eb3fb5736fc493ea` | 410 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=16/estimates.json` | `526f8e555b9b4f347081983f8083b4da07a31c0af39dd669990e8694b923bc65` | 801 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=16/sample.json` | `0772b6afd34644161c3880e958dbb180223f7f85aca3c05679eb4148f18c8193` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=256/benchmark.json` | `4907ee0e1720d9aafedaa686c1f1a7d1678fb6db727756e3c89c5293809f6c52` | 415 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=256/estimates.json` | `583981f7ff053eea0b71199810ce99284ca88ea769e34d3670b20fd0ad0b75ae` | 799 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=256/sample.json` | `1fad7ad79ab5cd4bd63cb8ca03c4014bfbc00ae6bbcf5facbea202df7e49cf08` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=4096/benchmark.json` | `a02558e15328219257e16546fe740fe173454ac95a83ee8f47d9c257159deda6` | 420 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=4096/estimates.json` | `8fa72e793638b82ff7215347f97d1b9a1b1bdae4cdb5cd95ac5c65f13709050a` | 767 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=poly-remainder-scalar__W1__warm-reuse__T2S-mother__B=4096/sample.json` | `57925140c5fd7210c72523518fd0dbb70b91549249526f9db0e35609828714f7` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=1/benchmark.json` | `1d2e89e70825fc0fdf7c8871416e6547011455f3222ba288a1c229104b2e280a` | 346 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=1/estimates.json` | `50476781dc926aa63859b82de68d91f5a0d32c3d2f90655e681498084ae2b9f5` | 991 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=1/sample.json` | `ad29426fe63166b333f3b65cb0b4f43b9a970e873f4ec4334ead9daf880fd9e4` | 2010 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=16/benchmark.json` | `29a0df64f9afb67cd70de616046af0f095d8c73cfdc757c7eb969fc9c17dfff3` | 352 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=16/estimates.json` | `18da7f9e02c9f434214e2a8c40af8fb44d4f68f3b3d0ed5bee35e15a6b6416c8` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=16/sample.json` | `d6085a7ab27909e2ce2188473bdb671528c789c80fed01e097d5163824cd4d85` | 1901 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=256/benchmark.json` | `24601ab59e96f3f96abc4f03aa35439680faf6ac7148a30b91391e713ca98696` | 357 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=256/estimates.json` | `976b944d17823766d1578fe7311ae73306957c65cba7490387e48bffeb8a104a` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=256/sample.json` | `fb96abd0f56d0d41b26684438bf3c842b23603388688a16a7bd6228c07d3aeff` | 1744 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=4096/benchmark.json` | `68eab3945c5cc76df9310a6934d6a942f67fea689b3f85c43178fd0f3a533b99` | 362 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=4096/estimates.json` | `fd24e065643090288f27a16492f1d5d4465216e8bc054975fec5fb37f2524181` | 772 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B2__B=4096/sample.json` | `4b528b3a2b9d5226de7aae1906118d6bfcbfbc35d654814da5e893394829e9ab` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=1/benchmark.json` | `e88926017c6c566491603f9ec7532406e11cfbaf978673c47c779d9306b4b7c3` | 347 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=1/estimates.json` | `09f1355a25b7652302d0fb95c5382d4aea3d4aae4c23e8d47498036767ae7920` | 990 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=1/sample.json` | `79e6311690e359f7911003b9484e43f4c0c6fa87f5482a742aea66a8199bf4ae` | 1979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=16/benchmark.json` | `e316589b86d07083a62a1cc04d1e029fda03d3f957eee9b2b588a42db4d224a2` | 352 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=16/estimates.json` | `2bf8e81a278d45170304fec23ceb87e5f4e2d7dd1808eefafab824991cd975aa` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=16/sample.json` | `fe3f98c9be336b9ade688f99caadcc5368dcf01cfff3d16bba26e00f77d21883` | 1857 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=256/benchmark.json` | `f01eb78baaed54146bc85b8a20419d564736a68d74fd52f9deab3682584cd05e` | 357 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=256/estimates.json` | `63d4acdfda2bdb975ee78f2348529949bc0d3a11045e29d334b08ca1b3532249` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=256/sample.json` | `61334ed1ca203bf92531119467fbd9e4b5b8bc4f775e0f8c23d1b27e2b4b26b1` | 1742 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=4096/benchmark.json` | `cc2a55e65f60f7c71f40f4d90cdad37879b83d4551ed51a44c1093b51cb53aa1` | 362 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=4096/estimates.json` | `3d7ff67a847bf4442d0f47e285aa9586c0624aa270e134ea15cd5b5c66924b04` | 798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__B3__B=4096/sample.json` | `c5212100974deb040fb385829a16df91da39958b185c1851d9e3a158753c220e` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=1/benchmark.json` | `edd23b2ab338ebfdb597962734236cc6c8406fae1ff092e8421700872a9af1c4` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=1/estimates.json` | `f46d3834a678babb5f53ba37655c1fc0108f03ec5bab2ebaca4a3ef8e56eda2a` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=1/sample.json` | `8d6fb6a51296a4fd2bebd7f4b43383ebb582482eccc14a4c4f06036641dc1abb` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=16/benchmark.json` | `bfd1456d00296722dc4bcb3d1fb90c1a25c6c1051ff84ca58f447d0ded35f373` | 387 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=16/estimates.json` | `1238a01621b9c37953ae81ae909491f4ab6ca0ddca6a1c367cfca83526c0cf9d` | 780 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=16/sample.json` | `ed5c30dce5cb0d02b7df7fc22da8413001953e6c83f3462c15d873c542be1b00` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=256/benchmark.json` | `1cf38b179980cf5557011be2938c723877460f86f39fe64b75ca9f56dcf6d3bb` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=256/estimates.json` | `89c277790bdda2012efbd220986a9b3fa5999bc9e67e40af8b4d20ba5165ca30` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=256/sample.json` | `343b41d2c42c089b81a5accdc916731cb9026ca2229f0a78e79248df72a27784` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=4096/benchmark.json` | `0ccf025028dfa79181cd336e83b22a783cf4c2faf782410bddbf40ed56b46bbf` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=4096/estimates.json` | `f2ea5ca45e4fac0d63887ed61e55cbf28490760e8acc24b91dfec761635602cb` | 767 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2N-mother__B=4096/sample.json` | `68c4f07081c3222c3b424e21ffe3ca30508ee433304d834672a713eac6414145` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=1/benchmark.json` | `32791b29fdb6f11de1e081a24fb33e9b221d948b6e6afdcabab0c910e87b294c` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=1/estimates.json` | `85b0208d2ad69cde181f407b69316149d847fd5ffb26d117c7b160dc05da3813` | 793 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=1/sample.json` | `5052c80825c2af7178aab4621c82b4b1269dab0e066c903f3d4d7e60518a9949` | 244 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=16/benchmark.json` | `25b8e56ca8c9b2c2597e2a8cebc8ee1e22c14431a41019e72c398524adf45467` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=16/estimates.json` | `3d01092569aee0a41e74d82d70ea4668d0d63e5cb27d5ed3387d5ff718b428d1` | 798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=16/sample.json` | `a4a40328ff5e4eb59f3b80d6d62ab5b57f193739827f115296b6db509d616aee` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=256/benchmark.json` | `24f081a8da027ef868ea691c375d5ffa1d5c7a57719266c0f036d45daa65a3db` | 391 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=256/estimates.json` | `b05d3c93dd2694b708fdd7fae4b997756ac9d392aadde8b27ed6036a2774848a` | 794 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=256/sample.json` | `b751437157d7f0ec405da35803b41751434c7041e12d92b9c40e373530c5e4df` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=4096/benchmark.json` | `668e0c883149084a90f2af942f4d6e8b00be39fe9b3a0dd6399ec1627686de21` | 396 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=4096/estimates.json` | `03f8db9bc5c85fcb484ae4212ba6cf7b5e02e7a0f3c37e0220d8fa7f576c674a` | 770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__family=table-remainder__W1__warm-reuse__T2S-mother__B=4096/sample.json` | `7211c31703d0200c07e3f8f9aa4c5887abd46706279573202a337c7f759b9433` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=1/benchmark.json` | `d176d952e70c281817f9d12d01d80c5a1da84e4b1778f400e68b7b1f01d01bee` | 377 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=1/estimates.json` | `b2f35e7c82139608e2a0e96432e22cdc8bb41466e4ef331aca1bfef20e823f3f` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=1/sample.json` | `d5dfcf7329da91bc891298718aae9304f4e857b3a180661f7de3a2b66ef783c9` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=16/benchmark.json` | `bc6571cdf2b6f8bffb48d7b14266ffcca3acde33cbf6b95be48bf14ae12e8824` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=16/estimates.json` | `4ea23e95836e1d0c007cd484487afa235699062db58ffec9c359e05df5dee4b5` | 803 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=16/sample.json` | `69073469996e3072abc69eb010c179347108456a059fbe4eb462334221f01256` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=256/benchmark.json` | `161d06d64aa6a8fd3754cf739a14ad1a576b80c497a9b6933495391253e02443` | 387 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=256/estimates.json` | `b6a95e70aa29b9072297d0c43d089dc08623de1c3d33240baf558323514b00e4` | 782 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=256/sample.json` | `def2f38473964ad55a3bafa63335a0df28264b6ff550ae828f3344a59c4e865d` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=4096/benchmark.json` | `bf8fe5b778d4e0d328dbfb328121c9e30e86dd9e98060c5522325416ac399e45` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=4096/estimates.json` | `b7716708bb6e463080655c9aa5984b8124f0ac49f0eff27bd6499c58b86b4951` | 766 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2N__B=4096/sample.json` | `46cf990756aab4ca58c056bff2b4032c54b3171f9024391780163d438c19f1b3` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=1/benchmark.json` | `9014b87aaacb20025a961f7d359d42e4af5c4df6dec0ddf772b833d546c4f8eb` | 376 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=1/estimates.json` | `47e94e08ee07129f4d804eceb26db82ea195dc66eace76fa2552e796a3dd5cdd` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=1/sample.json` | `d51008cb39f5d203bc6d0ef428b7c11b395386c0e17500afe8c8b1becb5d4614` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=16/benchmark.json` | `f62a93edf591a6a84e3437eaf8fc7e2958619a62ef60c2480398b02a1ba5c3c8` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=16/estimates.json` | `520e27eae9afdf6d4de43f45109afbc32fa22fe35163c89605cfa8dfc6ee74fa` | 799 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=16/sample.json` | `bc725a89b4e4f627c614963746897ae9d48429850a9879f3b684860fc177998e` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=256/benchmark.json` | `5ec1b1ecaeb06ba6dd7f9007bca9c7aa24cb1ed60f4f7c49199a55127bb4ec65` | 387 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=256/estimates.json` | `1754a7fcf7ef7ad10447c457725eccc5f86728ba388d823268c2a7bcdc75a763` | 790 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=256/sample.json` | `efb0c8d3ca1924c58018392c3fdc6c31846fa0b76bbb0219473b2aec22685317` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=4096/benchmark.json` | `865a1f9ec80b36cb11c70facbaf1038fd277308663e2be518274aee931fa25a5` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=4096/estimates.json` | `32d5b265295a6224285d1f515fc26405c047932b027c826de002b62e6a2ba4c8` | 764 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__fresh-alloc__T2S__B=4096/sample.json` | `54860e7324c8d1e98b5eccaf2d2bd72071d2aa96e3416419701779dfcfdd510c` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=1/benchmark.json` | `70265c3e69343c548ef244169dd8d785024bbe275b94173345bc0f6998fe7e00` | 373 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=1/estimates.json` | `2aa0e56fde5ebae15c0990853b6fe9da08b73faebe834f870145b0f495743b93` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=1/sample.json` | `ce03c06686269eb114d69eb0bdd03f3407c2c8ce5564718f2db5fb0743786be7` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=16/benchmark.json` | `80ef0681f78602445822cbc01b66c20d05681abf84afddd02448916b2bbc84a0` | 378 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=16/estimates.json` | `b38dd0e3584235930af31b20c15efc39ffa1e3affe0ba0e4303a6103427fcdab` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=16/sample.json` | `33e78a9dbd941ad08dd53451efaf7b58c8382e284b45b5a6ac2b243ce2b646f4` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=256/benchmark.json` | `425019acffe12b79aa9d18dea4b8a841b1bdf34c789fc4c9725ff6798781fb2f` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=256/estimates.json` | `8e799692e28ce7b9ff75f1ddfd675636ecac073619a5043addd3604ad5b823b4` | 794 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=256/sample.json` | `872903fd05bad288b5b7c0b9712bdb5d3e290604cd2d7b6e6fefdbd32cb77c46` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=4096/benchmark.json` | `6a1c6cd9a74793f69b285ca4d6e0587a314f4c7c5785de7c13266f82d6371ca1` | 389 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=4096/estimates.json` | `444731d82e3c89f4bbe0267f0f18848d40742cc733621b41d321ad6ec5c1e5aa` | 770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2N__B=4096/sample.json` | `eda61c10cff2f3979b8c2805feb3bae4695adf43e15c9c4dbd93e9186418bf95` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=1/benchmark.json` | `9e0b3cf03178a96d9dc947205f8aff2ef02bdcfa14421018be77283818ef9476` | 372 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=1/estimates.json` | `5b8ddb71f336d68e6ba302cae6246393f807ab0db768daa91abdf36daa4e97a5` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=1/sample.json` | `2f35e13d361f30d384c2c8da546f9dfb466e1172b1411c038c9f6db64c50e3d8` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=16/benchmark.json` | `89ffb08efe474cab098a63458e0f86af1a47b94638c6d2a051d87461f9e6acf0` | 378 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=16/estimates.json` | `4da6594ff7a7a9e844254eab9f4b05aec17415b258e762339df1d742dc95ce29` | 799 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=16/sample.json` | `eb80c02608e7158c16d7d93149e2247e667ba26e53ca48fb9bd19361c6e9a9aa` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=256/benchmark.json` | `5b57c7b05d0fadc4ccf1ab5b58437067f75f01d0c8ff0cfa9e519f2b00facf78` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=256/estimates.json` | `e64f4e93c966c04d8bd403feaeacdf39b5b5e9934c8ebfc046f3a50771e09549` | 795 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=256/sample.json` | `ce4f22ff59b53dfb2128fbbf25d666fe03f6bbfa8449ddff1f795b636f92821e` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=4096/benchmark.json` | `2097f8bcc33138b07fbb76dd163ca5334649626ff127a5166302f5930cffd551` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=4096/estimates.json` | `61b1281d6c3cfbef052894ca0a9968a5c6567ad86785f45fc94d3023c2549ad4` | 767 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__route=shortened-restriction__W1__warm-reuse__T2S__B=4096/sample.json` | `49a028e7cd6f67050ecbb68aae9505384da4f05c3b18cfda88050c65df289285` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=1/benchmark.json` | `66d58ee736547c484442dff67fa300d0a704e44c06aabb95304eeca785730d70` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=1/estimates.json` | `a1b93626f9804bb81448c596139d748656499c1ec90bac644c936c353972ba9f` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=1/sample.json` | `cdff370268ca46a6570330a37fa7c6315cec56ef9949b1167f7404f2963dbf68` | 2038 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=16/benchmark.json` | `cc27f0732544531ac144852d42d1c219d5b79296087423d05ff3230c41f04d3a` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=16/estimates.json` | `998f4185e548574f1fe4f183734224a30618d40a45c00e6dcf17eac94c7b2c2a` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=16/sample.json` | `f321bae5cdbd78392fde24a073d713d4362a065bd588e90ad1f43759dbfc6d42` | 1931 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=256/benchmark.json` | `74d9f11689e3440fb7f6802306ba0ad3fa23fdc95aee551861e14199aa3a0970` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=256/estimates.json` | `65416db29d12f925a140c92dc9761bc36eb8fde55f2ee5ead58ffa360d5b7499` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=256/sample.json` | `4a27bf0e873397ca9d099e47a3e2077165837e82d1beede979dbb1d88c0b5475` | 1820 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=4096/benchmark.json` | `ac2a961eac0c0d7e28ca82765663eeedc5da4589340ad5dd962356e16a5a4ef4` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=4096/estimates.json` | `a9dbe5142ec110c973a84000d2f5d3f71297453383ca983c15d3d7185b25d1be` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B1__B=4096/sample.json` | `9eba4f579cf9c00a3971474b9e74722611125dee31fccde1ebbfc5f391acdc97` | 1701 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=1/benchmark.json` | `73ce54255ada13d5b77f40731a37cb7b7b0abde5c48ad224337856a315da2765` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=1/estimates.json` | `90471a7cef4d4fe6fe2674053a406f9e1bf0b7c3f21b55d9d3e03f79e299f5fc` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=1/sample.json` | `521c445891db3eabd6036bb71db9f6b14f02a6e3b6037d511ea0d882537a487f` | 1995 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=16/benchmark.json` | `a1365bc98a25e1ef8034f9e404eaa44bbf9bbe9942e76198b2a97598f4f615b6` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=16/estimates.json` | `d5179b9be3c8d208d5dab0f8add288c72a2d1d9f92a9c8696e2ce2fb6cbf51f5` | 977 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=16/sample.json` | `c7fb02372ced752574753c074e3fc6e6f4ab910b4493b1df3b5388fcdb78d08b` | 1884 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=256/benchmark.json` | `3532ab3eb5cc63cbd86b3227f97913023fac9adfc19fbe25e7f57a1d485c45d7` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=256/estimates.json` | `ab1e240dd10345d832d751a4c577d0e172a13a533927f61985c2bb883756400e` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=256/sample.json` | `e7b9a55d2ed4e927c8f2355cfb41e581a7a936c07e08f2c4b5188ad852dccdc0` | 1727 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=4096/benchmark.json` | `0b158226cbd3312faccd5d6da124afa2a764dad97c2b322b248b2d2d00605a8d` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=4096/estimates.json` | `7fc792b4ecadd114a6b9d6816663293b591d71f485763bdca24eda142a9dbe62` | 775 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B2__B=4096/sample.json` | `e9d79bec90b7b2ed9825333df0f1359bb79a856426b9c97dd9077cf67844a48f` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=1/benchmark.json` | `ba0ca69596faa926ff70abebfc3a116600cbdf71d1b6979acae5a2e4ae789567` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=1/estimates.json` | `97014577bb5fb1e6b9a78edc4dcf7fa80eeeb66b6d8a956a8638a5a4a6207322` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=1/sample.json` | `105af469a40b45d9e18d6640d9a6a15989af5fcdd31b888c0bd0df5ad226875d` | 1945 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=16/benchmark.json` | `5981a3d93ee4ecdc587a52f95cba7a617fc5f5b2aa8096d5a702ddd5376c4cf1` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=16/estimates.json` | `ffea2940bf0ed5133f3e73a363b2ba478a9fd58d3a69caa2390a4c4cc39ce42a` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=16/sample.json` | `2c6822e0287b460d047baff5c8b03d80d3a182c2b03a92dae5cd7477fc9c1322` | 1821 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=256/benchmark.json` | `69deada718d03f7cbba31fbf03e2023294e27b0b6eeb0bc484552f410430bfb3` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=256/estimates.json` | `ea32515152dff05197ad2c31147133a34458d7afec972311fcd259dc99a88c28` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=256/sample.json` | `797d3a11027ce854e6f9a33cf57773aabf5575865df6030b9ce480f4104dc6f1` | 1731 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=4096/benchmark.json` | `1bb1b34bbacaa0064dbea0c5ab16d2bb4bd2710657d9d77cb6c2e5f478bd3bab` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=4096/estimates.json` | `1c3b207b754967a2c62c86d1bc8243040a5b71dc357b34cafa4f191aa5269448` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__B3__B=4096/sample.json` | `c2e42b47efcfde121f7f538a22a6f7b8d3b45690d061fb3b4327803402a37e34` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=1/benchmark.json` | `55ad5ed04c0b3ea6822484a1eeea2155e108a262128151bc44b0ad0a3c1fb0c1` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=1/estimates.json` | `7fc81e5362489c4466eae8e6451d9ff70d63c0285b65c90316023bee8b603587` | 996 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=1/sample.json` | `9087cc6160f8de4bb6301dab72144ba2db61392220fb89c31304974cc5848583` | 2023 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=16/benchmark.json` | `26c3f65d02a8d83b7d389f7602ffcaefadb88fde243f5706cc7ddf8ab47bd4fb` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=16/estimates.json` | `493bb2031b21f88b626c4b83b0dfc2ecfe6c59f6224e3cf79fbd8d2a1f56fa7c` | 986 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=16/sample.json` | `8fad04bdc936700224f4d326d48d4f589e849aecb592fa749702a77248fda26e` | 1920 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=256/benchmark.json` | `bdb017ce68871c4e99fea83ec2a9afbf61d99bb9e378d44339ede0dd996aea9f` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=256/estimates.json` | `79111f5bf74686afbeccffa940a4807273e206da63f3149626a708b309001e97` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=256/sample.json` | `0ce41d8c033dc59099637956a6c8904da54edaeedda7bab28f6a5c1d5ccdc6cd` | 1808 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=4096/benchmark.json` | `45bb3ceec5f50b3a7013001dc49b190aa59df251f5dfdfbbf906f8a39e491cd4` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=4096/estimates.json` | `1c89788c670be3ec7e2c12a99afa2af0c92ad74cad94cdcb249880f595ddbea7` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N1__B=4096/sample.json` | `375f06c26df3b04a1bf03637d645ac96fa054eb40dcb89b2831a2d9e8b7e11bb` | 1720 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=1/benchmark.json` | `1abe7808b145ddeb09b9ebf82dbefdd658487fa3114615ceb4c9fb1639474c76` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=1/estimates.json` | `0a029af37de08010bf5e731c1d2732b0408d7a1a9278ce5c57d0d8b28bb9420c` | 988 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=1/sample.json` | `4769a457e8473ab0ede161b744e389b6202745e9900c461dc556e065b03fd1a1` | 1994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=16/benchmark.json` | `1215f6fe2ea678d7ee686072ef8381e9ab6ddb92c5864ba1a0928e76ac3587eb` | 387 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=16/estimates.json` | `fd82ffe453d599dfffbcb14b91863c51e2b34920efe1d458028e3dea3a65a656` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=16/sample.json` | `a95d1d06bfca0ef08bb35d6f401153fcf5a4587f4f3e4318476f012e9aa2fc6e` | 1874 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=256/benchmark.json` | `805781824347e12c3b290c813bf9e97f9f80d12305bc090edf86155fa802262d` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=256/estimates.json` | `5b91de551178507cdd3d3c54a11065da68409de5a8f53d0a31abd0f883e311ea` | 966 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=256/sample.json` | `d47392b6a57f0aa6806e6f32e13fec2bf499864439f6f407bb7cb8a3103e8023` | 1741 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=4096/benchmark.json` | `db2382b744882d61d01c94488e704773dabf7e3dbf86c839170f459f7ed34db2` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=4096/estimates.json` | `db7ce055c62465bd52034264f391dad3ac996a0903a795a33f8a067e2b8b8222` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N2__B=4096/sample.json` | `ba9ac7a61183aeeb3a663ae98cc79ce57f26e5b95a4fb748998334baef329e18` | 1666 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=1/benchmark.json` | `6a44423bb7f5495c7f3f50f292529099608d1f4ea5dbec989c67674b13152363` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=1/estimates.json` | `4077373aa92b426b58ff70f115fb288bc05ad79cea3362c85811e0a02dd4f255` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=1/sample.json` | `ddc7835a333f680e2cda70a222049b07aa68605ec2ff0993808f108bda822131` | 1922 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=16/benchmark.json` | `8a47c5eb000899fb2ab4bd2115763ec88a87e93050b4b39da240c9657f450d4e` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=16/estimates.json` | `750035a4b25da32d1e67c6411df125ddc0681ed026421690a88f03f1142b056e` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=16/sample.json` | `e6ff4dd0d7d93fe283aab68ee63681d34c555e49dae01a628285e3d36bf87eee` | 1798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=256/benchmark.json` | `ec38f79f3c8aa60488cd0c78518f50943c70868d1dc22b7110ba73aae13e1907` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=256/estimates.json` | `0a72426cfb8880d9bfe43da5f6b535ba9b80f0eeaa080200f54e70df769ebdc2` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=256/sample.json` | `03825ae7ef566d5f481eea75c3aa832235a91ec3a562517171390745f17037e4` | 1694 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=4096/benchmark.json` | `bcdc0bf1ed0f8623d9978261bab004abdcf9cc1e2439f88282c3b9cb350d3df4` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=4096/estimates.json` | `1f625ffbb9624382c4a464b03a2dc1107a0376e41e09c6a4f5c0422512d27451` | 788 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N3__B=4096/sample.json` | `cad1f647669a52d14343452135c495596d6edf73320cce0016409f8b5890eb20` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=1/benchmark.json` | `47312f8750b1823d64b03fbfa07affab3c83cc18b4ce7d0800671b9a1855aefe` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=1/estimates.json` | `fe2fd4e32cdc3ad4fbdffd9f90eef10a1362ac5ffe265d7d2a2d5b0ada65ad1d` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=1/sample.json` | `b36b9ebe83c1ab4e407bf4750d6fdf2c9cd5b15b15be67af0699865318c68a89` | 1753 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=16/benchmark.json` | `2dd960141d362034a781a793a14e7d0c382e04c378e1194601d7bafcd309e9ae` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=16/estimates.json` | `1322c247e75e8d98bea19f3a0a307452230db99020618d9f01a3cbccb543e7e6` | 971 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=16/sample.json` | `fe5f8837d9bc17594c2cccfd63c0b4e14b1069a06e42e5ea281a7ec733a9fafe` | 1661 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=256/benchmark.json` | `4d92e9b17218f44d9ff8b3af97e3775042d6dfd56ee91dd958d97d488556f43f` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=256/estimates.json` | `f6fba6b5cc95b3e913caf7cbb31df09fb1e7840e9c093aff118f428a64b4b8ec` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=256/sample.json` | `54c1fee4ec8323013645d8eda9f4d42ca03311385e470bba070983917bd1db70` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=4096/benchmark.json` | `7a4aad658f904ea8a7c1b1735929d652f0817014180bcde0354f27a8ddf8918f` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=4096/estimates.json` | `c232625eb78d8f87337e68d79be5c1d4254899e4e295d01d83ac77e56125b984` | 766 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__N4__B=4096/sample.json` | `73a08ad6d4fc8f7a309299a21f21c144345ea8f17ba3fcd2632ef1c1d28a09d9` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=1/benchmark.json` | `8e9694c729900a14f67f85d6dd874ccefa5f399969838ee27e3ee6de645f7f52` | 417 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=1/estimates.json` | `6e2852f432b39d867998db79b555fe7f38aa2237388c2c91709a51ad86870bf3` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=1/sample.json` | `b81889f29593fcf0470f20073969c4a9340eced3941fbc7c526bbcfce7a2d509` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=16/benchmark.json` | `3de1f9975da9268c0546f0f1b43e1a05bee19ec22f83894c78f60092948ea019` | 423 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=16/estimates.json` | `411967c3b1588635078609d0b1f00b3e3d28819b31e53a3010f53a89f58953bb` | 805 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=16/sample.json` | `f29357b5db5d31ccaecca7741495b18c8e849820e7fda63d1f23f5fe4fc9d466` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=256/benchmark.json` | `2bdcdd56be981f9d32ec5e7ec5d84893e99ad1523fc3ead457c7d9a2b45241d3` | 428 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=256/estimates.json` | `10afbe51859403c2997082a5bbd216e77df52a0cfbb3bfc32b24d8f2f30d4cb8` | 781 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=256/sample.json` | `c94d4f47f13da9faacbcaa3bb8288dc2527ddb238933ac1290391422d7bd9d5f` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=4096/benchmark.json` | `2a67d7800341d91270128922364f604281d70e57964e69a4fd9bc89c6fa7d3d8` | 433 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=4096/estimates.json` | `7219422d9ea24f83956825fe3b5a3e101978a41873c2a95de781f971fd30420a` | 771 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2N-mother__B=4096/sample.json` | `20b55968077a0fec377e380f762999ccb26977913b1f9878aeab094a0d224b45` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=1/benchmark.json` | `532576f9f1694d8c77325d8996fdf7e1a7adf2dc7965dc10c1a5060a9f114153` | 417 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=1/estimates.json` | `0507e5bca969c08c59de8062d114d1b46fb09838bef0742bfc669217f7ba9252` | 802 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=1/sample.json` | `be7816b8b94031e76e287c899c66f2be1bf132437b7f4a8042eca4d9a11bd243` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=16/benchmark.json` | `dfe34c2bfcf992f26a04303718eb9ac857fadbcf6eae6d41eaadc81d8ea1bf00` | 422 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=16/estimates.json` | `f4014d72805e7e86ce449262a00586dbac9acbcd48f3ab1c249b928dd988c193` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=16/sample.json` | `1eafb79bd9991024134e8b86c904f11985ce2d5744b1f94c6d1b1f7c6715e4d2` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=256/benchmark.json` | `03bcdc624d74774ebc34368edfc1282098140295d302be3a153c85ac20436864` | 427 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=256/estimates.json` | `588a6e422dd581ebcae988e1bdfbd23e72b86285f513b960bcd5bd2e057b475b` | 799 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=256/sample.json` | `21532776d7e58342d3a0520a6d444df982348d992a9654d73ac1d4fc441fef23` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=4096/benchmark.json` | `7db6c68771ff86b6c46dd1aa1aae0aec543566b697f0eea929e0729b0bb942df` | 432 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=4096/estimates.json` | `20144abe946381f158ba60127d1c8d5978676945a867dd25ef430203ae8a0208` | 763 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W1__fresh-alloc__T2S-mother__B=4096/sample.json` | `118166cfd0353ea70691ab42ecacf9b387f8548c374eb91777f231a1bf970917` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=1/benchmark.json` | `96e4562671b50bdab237950cb13d1976e459c4668554c7a15fb26d3f68400ee1` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=1/estimates.json` | `e4bcf872f55ea3b6a490095f5d6ab10a6eaae2b4615fda2ea6bdcaf2d8de7b9d` | 989 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=1/sample.json` | `2382e197e49bb7855a4ec599492b81b9c23eb75d7f42243104a9efda4036fc33` | 1921 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=16/benchmark.json` | `c532544c78d8a7b4315a05589c27d074da02bff3e2dc9166c53b79a11e016876` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=16/estimates.json` | `b6f646b006c59680bc32b1d1c65cb4a42377474d289529e18fc810a0c53c9574` | 977 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=16/sample.json` | `6889ed7bcf4cbcd5955be27cc9104bc69acd7bf203f6500a38684c19d39c1ec3` | 1841 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=256/benchmark.json` | `fc5e4b3610b7a0605ac16ee96e6885650df561293f818dc45f844a9bde4e5dc3` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=256/estimates.json` | `d32970008eaca28692dbe9b8b2ae99bf1e4a89c54a48c48fcb744ffc9a994348` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=256/sample.json` | `2d13de204ac3c2105e92fd6a2871d13be95ce732db3674e69071d123d19d29ca` | 1814 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=4096/benchmark.json` | `19d9ec569ddd429569ba4f5a916c5a29040c19f6fc4f90246bda23b366ef9172` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=4096/estimates.json` | `fa065ab7454e40fc7452eed9424c26dbc9ffd4dc1b9d5ca05d38599ca1dfba31` | 970 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B1__B=4096/sample.json` | `fadbc649f5a27373d0ab106e6aa26999468f4b6849b4ecc24046e6cc513f713d` | 1730 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=1/benchmark.json` | `78c2e3e981d4d8f8d26fb8af6d6ed47ea96da1d58c4986fb94214309cd84e5a8` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=1/estimates.json` | `9f80156d1070e2f1e739c8b783d9d3de1a3aa2f809b7192033b7eaf23b8dadb9` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=1/sample.json` | `909e1ea3d88087766104c57f157843895671a7878a3ffeaebf1a98d0f67fb06a` | 1808 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=16/benchmark.json` | `332523f1ca7e454acd000fcca6487b92de9d22fab2f41dcb1ceca3ce4dbcecf6` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=16/estimates.json` | `d24664440e3aa4440628bb0d8f41cb541f0606a521144d8776a123487ca1432a` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=16/sample.json` | `d7ea8e30c34565fedecf0c03a5cdc4742e968605972113161df0d4506f61092d` | 1788 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=256/benchmark.json` | `bcc98bc685bdbdf03ae4612bcb9b50fbb7e05a53056ca711b858a3d213f4c4a1` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=256/estimates.json` | `29a69be0c441ebec5b4a4e5b4679c6ce5222eefdfe9019fba5bae92862e25a1e` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=256/sample.json` | `391eb2643f9b105b3e16ce6c8cf70ede0f591763011241e27760d7cd7148069a` | 1770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=4096/benchmark.json` | `c57671e78274dc3ccaaca2dba5fb318fe3d81f0060800db6c00efd07903de87d` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=4096/estimates.json` | `5db181ca31443681e189598386726ef8ceb20ab29e19411c03886704f07b1386` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B2__B=4096/sample.json` | `fbd7dedd066c410d02ce8fdb52d1008c25347bf7cf7651719743b600705b4944` | 1720 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=1/benchmark.json` | `e53a900405d3abffbd7440da63aa63b2e9d316375e5112f52f2b5c3eca595ac1` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=1/estimates.json` | `07143fdf4c1824edce7aa96f2288cbf70198e4e438076136747d72a378714686` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=1/sample.json` | `65fe2d294ebaec33744cade456ce776d335bba58e9355149a7f0648772d5c858` | 1808 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=16/benchmark.json` | `2fcb6bdc1a0483fb5f5d1098d50d557ca33062f500cd3aeea46ef62430f59b91` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=16/estimates.json` | `ee31b85b91cdefefaddc844d44774993ad446e5037e6035d458e631015cd3ada` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=16/sample.json` | `e83f94a0e8d1241437e6509e68fc4027b99931abd02c3a009c881819d3083338` | 1795 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=256/benchmark.json` | `145c28f300bdba2328bf9dda06f4b89b5e2789cb09f7542d19735e0f17268b71` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=256/estimates.json` | `a95700e341ca7b35303aa9b36e91595920f48c519371117af2beabbd3c5c897b` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=256/sample.json` | `cba4a36d88f80808eaf98fdb1c2543e0cd677fddd31dc1e037cc17d5af260a51` | 1727 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=4096/benchmark.json` | `ebbe39b22fa346f8fe001d8b3b82d553052cb2fd1ccd05123494dfacc9c6f476` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=4096/estimates.json` | `890e250c2a07aa7512f158b368ffaadd558e72922b6b9ff31c1ad90bc073edf9` | 988 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__B3__B=4096/sample.json` | `5fc637cfb63bba4237ad9dd00dcc2520e06e3be46ff0d80e2a0376f3ef8e490e` | 1668 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=1/benchmark.json` | `aa8e480ee5f40282a267c668691d1a41d790367d05470e564c345dedf6358b5f` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=1/estimates.json` | `ad42b966f544e039c244091c33581779b83da2f35b4bf5ffa6c11c729ff9abdb` | 990 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=1/sample.json` | `cd251e024c0821d7a5fe11f7f2107dd8c3d47664e55dbec4bf97b88de78b1b8f` | 2004 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=16/benchmark.json` | `5d8fdb0adfcd29e7f5523a6ec5a4538ec9336625c32409f650123b1e67846233` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=16/estimates.json` | `addf89908e80e81b2db3fa6bebb0297598ca08775bb1102e8aa95d2a5ea4888a` | 971 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=16/sample.json` | `11eca4fdd87bebf0b64489c90c3874e698ae98c7fb0ff521eb6e7c572751eacb` | 1858 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=256/benchmark.json` | `73032f2a42924432185f9e3736182ab8d3b1e8ba69af35592fe8b123490de412` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=256/estimates.json` | `d779c1e4eae9c56638ca461219003c67c893057251be9fa741f67469c05a0470` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=256/sample.json` | `9e08a2b9c8dea762cdf9ae7e43caefe84563c3ebd84ab66072fe46cc602ebe29` | 1814 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=4096/benchmark.json` | `62439e9db6bafea1fa1251de75de5d3567280e4f901c412c9899825db9fcbe40` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=4096/estimates.json` | `53c3fe4adc17325657622d7af2d1075dbfdca682179a5abbcacbfe3e7c09aac6` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N1__B=4096/sample.json` | `1567670cf5326df9ada5d4eff13f64bf4af3e495c7ff441728a92f41803ba0fd` | 1709 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=1/benchmark.json` | `c0b3618c4d6fe0dc05c83c5f2dccfa1f1863c1a7656a83eb58427351ea654792` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=1/estimates.json` | `6dfddfd74dc73cb9925b53d8a398af3e8bf780d02f90d3af8d1083e6e604cbaa` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=1/sample.json` | `077ed8f891e68f92f1bc78fe852735c184a00118a81972dc8f9277d1edc7de0b` | 1958 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=16/benchmark.json` | `d5d11dfa48f2cc25fed969e245f7c1e73055487c98cfe4b7c741450e0fd1debf` | 387 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=16/estimates.json` | `9f3cb4c94c9441dc8fef5b01e81636305ea69fd09ab021b1268d182806a69934` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=16/sample.json` | `e99444d8190be1f725fec9d698c689c26f68775e3ba1ab8581416c68e73c1b9e` | 1844 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=256/benchmark.json` | `bb206a77619b3a0eb8cb93a5e4c97df8bed5afa1d8c044c23110c1aa83a6a88a` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=256/estimates.json` | `6c057b12e4d8af56d0d4ecd3086a7c853fcb95725948e442361dbaf55fea5194` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=256/sample.json` | `fbac9fd3d7564c0486ae1b7305bb3a0e12674f89bee7bc312de3c72838d63e2d` | 1781 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=4096/benchmark.json` | `8a4e426ebdd7436019632733b508c63a6b21d60b9e199dcd637be0ec16cc940c` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=4096/estimates.json` | `d09f6ee7fe4ed3f15f5bf6cda3c6018ac8f807d8b84a9ac895061fe6a5dd3a9c` | 964 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N2__B=4096/sample.json` | `32f58d36dc92ae7645fb73afd25371017de621100c37988c4178b78c3508c771` | 1719 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=1/benchmark.json` | `e760e7277bc35b1ee6711370dee85f1f70ed798105435327e9a59727273aaa97` | 381 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=1/estimates.json` | `00668bc8aa3fddc8f536a248a1e5e1e2ae7d84507dba943632af25c244dd7ec2` | 987 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=1/sample.json` | `61881f9d06b53e9f612da9c9734e6be98df30c33cbeaa8ebc1f78c898c10fdbc` | 1901 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=16/benchmark.json` | `9dfa07371ce61ae4116c299bec9bfe344424daca2dd210cd611aebd7af1882aa` | 386 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=16/estimates.json` | `5c6a4b5713ce02e807af0fb5e95dbafba921fe8b368c1da9736a5a41e197d627` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=16/sample.json` | `20f4976b9be0a459ac041d7bec9739f44f08c21a9e9e6f5bf455cd76016724ff` | 1777 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=256/benchmark.json` | `5bd371613f3863499751f865b15cda39ff0be831adb9bd0af16043097def51c6` | 392 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=256/estimates.json` | `5a79431ee692498a4205f65c661361471b79bddab8f7b1832a1b3a5bbb03d34d` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=256/sample.json` | `202bc3bc7fbe10c2b070f562ad55609b85047d86460500de71464018bc47b451` | 1727 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=4096/benchmark.json` | `1b9236e855f127c637f487ccde3deb5a9bad2c94b888799512b584bb2b6da71c` | 397 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=4096/estimates.json` | `3e17ebd529c8600afb712b7f20abf7005b8efa5f7a5046153c4e92b75756ad48` | 771 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N3__B=4096/sample.json` | `daeef943fa6814749fe62f2de33e39d50571ce3dd006ca1ff7b94ac05a8ffde1` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=1/benchmark.json` | `2330b32a076b2f54a604e6ca4f31ddca00780ec3a85b2e2255592910205e884d` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=1/estimates.json` | `7f73c80927a73dd9f5c57840710e7ae5df3389bc4d9992219673f40dbccac8d4` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=1/sample.json` | `7bc0e43d91759edb5df209bd8ca58744415064518137eaf4b9b7ffd16502c582` | 1736 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=16/benchmark.json` | `9e73a9bd8233f781b2abadf3bf98104d23fb34a0029509af3007f07ea4d07713` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=16/estimates.json` | `8b62ee387da9aaeb346d5c0da95fa78e9dd59d50487654e7f4fda8e94aaba77c` | 769 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=16/sample.json` | `a5bf52e2cdc087cd62b73c5c2285a53b5b3bab52a819ae47b1e5fc33d94e0816` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=256/benchmark.json` | `bcd406f97bece4388dba9fa0760f7c2e3fe34fe52b1cf6b69cf2b42b371f43d1` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=256/estimates.json` | `4e8791426b6888a997d8e39cf43f325935f239f8f949f0d6bf41d21682dc4b2a` | 767 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=256/sample.json` | `85538070497296acc4049c140127c015bea872cc760152e48fdf87a771f38ac6` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=4096/benchmark.json` | `632d57c12e6fe44a85daf55c81eaf22b6dd5da16278fb638dacf916fee87522b` | 398 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=4096/estimates.json` | `65331d59362d31dc21c97150940dbd854279a28e990acb3dbffc3c184ffa573f` | 774 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__N4__B=4096/sample.json` | `84c1926c4897b97205ade81a57f21e29b8abca5ecc99091c1faad62209cd524e` | 1744 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=1/benchmark.json` | `9837ba01fa9c7a35771bd76a7d3869fa490140d3b8885b87c6fb83aca765242d` | 417 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=1/estimates.json` | `85bdad7391c964ecdbce9f78766be233ffa488ece60c07a22e1b02ad1c542357` | 794 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=1/sample.json` | `b3458c451dbcc1d351ceb20d411ce69c4d88411e6f75732d6f46dd3a1d8817a6` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=16/benchmark.json` | `ce00de05bd10efe52aba48697d844b13f3f94a3b3922625f861c4d2352759ab3` | 423 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=16/estimates.json` | `8cd770507df12f2e54d1b640fbf50fb6854bfed68c7cc06d6513dfc0d3934c41` | 798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=16/sample.json` | `7cf8fe3f195503bd8cb2e561377d34cd255d3d9b0d7b38933d23b27dfd31bdd2` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=256/benchmark.json` | `45f456fb76e2a796f9be341e91794c16b2af1088bb28923602b2bbfb5d265ebe` | 428 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=256/estimates.json` | `127747077eec5ed033990afc25bf69ada2467b084881433f41a4fb5bd224aef6` | 798 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=256/sample.json` | `06052e53542de07a2a3bd908d53acc7b7e241a99804cfdfce5984c47862f7303` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=4096/benchmark.json` | `24ddad34993a5cdd914d42587b1f3e7fb5bc90be28f74b9f140b4b3e69040fde` | 433 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=4096/estimates.json` | `ae2b961665a35e3b0cc0c05ad73e5f587ac2e0772a1ed9ad8a8015cdcea19358` | 764 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2N-mother__B=4096/sample.json` | `d31f3466f627e4816338a8772e01c34c3b091ad60c195dd5a15c7f44a80b0be0` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=1/benchmark.json` | `207e0bc63888465a4f10615c30f8d95fb27761e61e48ae4866bf3986f5016151` | 417 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=1/estimates.json` | `32efae7742269b7a47e711a13ddc412cea2c66e65caed258f58949d9c453f49f` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=1/sample.json` | `9680922f2030ba3bf0fb10ba48d0adaa9c979cac5b5e65d53b7254c7c9719a16` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=16/benchmark.json` | `9d9d81d219623e7139f4f08bf87558e9ee78828b836e554202771a2ce978ca6b` | 422 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=16/estimates.json` | `1eea9d0fd5ab2475e0558ffb2dcb158eb188ed4823dfaa964dd71c9f695261a9` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=16/sample.json` | `40537bc8602cfd6b8cdea5186b46b4ecadb2e10dafed3899e30e6d2ef9e3a99a` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=256/benchmark.json` | `602cf34bc75ea7feacb3f70516e343cb638fbc8dbdb7c90afbf676591f16ed2b` | 427 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=256/estimates.json` | `976c1de9ec00447a6d33a4a8b6d141e915f34dd7709058c819fe89fbaedd9b6d` | 795 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=256/sample.json` | `11acd2dfc8cb8a4787fefaeed4a8d590de37e74f7ea6a308729b7cb1ea8e2784` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=4096/benchmark.json` | `e961e2e7dd37bc1cb91b28637bccc264c59c9766500b161e60e7278d2f9aef18` | 432 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=4096/estimates.json` | `2804b659274d041a18a3289e42a9e23e9a548c6a5a872acbc1449fcdeaf02179` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__fresh-alloc__T2S-mother__B=4096/sample.json` | `366f12cd579b86ec23ae08bd933bc3f030f26186ff740091781d102a59c00105` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=1/benchmark.json` | `603f1af264ecbb0b1d75d215a6614581cd19f0809a5f817bebd28fae5bc23766` | 377 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=1/estimates.json` | `dd2e9fab498acac82245b07d2d9471eef6a34a50a4c9099e2e7084b0c0d9a07b` | 994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=1/sample.json` | `e0e7637fab41f14f84e6a015ea9654b65ed9b7664a824a36efa03a98f5d1f4e0` | 2054 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=16/benchmark.json` | `69db673db2161bee0d6c27b10d9b528a3663fd0f21e4eb59a577caf7552fc11a` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=16/estimates.json` | `397cee06b23500352d3a6e8e950ffaf488a4210d676fafb479d3f7ba239dbd59` | 971 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=16/sample.json` | `b366749b7ec3579b5cc2a7b208d06fb2908a2f42336761fe66ec195c27479ea7` | 1868 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=256/benchmark.json` | `242df66555743709c66b93ce55c56e77194943cbefba7c3249b03d56310affec` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=256/estimates.json` | `d5b01cd2c27c8b98d01d0aacec7bcfd1a12ac21a6e6c3033fc2ece1577810d02` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=256/sample.json` | `a7c8462fc77068a39e85351a1bfcff9a689be6abc29631baf3219a0761e41c65` | 1825 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=4096/benchmark.json` | `494e5ce540f4dbfe8a49b113f01153e7b1e426865444ffac1c72d0f06a8a7af9` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=4096/estimates.json` | `6b99dea9225e2dff9be65c7af83e2e9a66effdc03ea335dc881701ce2e65c2a2` | 970 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B1__B=4096/sample.json` | `7a8600ddf2e41baab488d8edbbf1d07fa8431c8cfb2d68e4fd4581d1c488510d` | 1753 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=1/benchmark.json` | `0a13788589271267da8fbd87f4ee8346227aed4871a6d08aa3f50f617eac46ec` | 378 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=1/estimates.json` | `d0b57aa202e00617faaa9abf289c6d08835f167c427feb631d5390ab6d4a9ea2` | 993 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=1/sample.json` | `7d78b87cbeec5f263977fd29c51ec1d400fe9d5253e6def94dbc22fa36c0d5b4` | 1994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=16/benchmark.json` | `21f597a649a68ee36d6c199e0aa8e08c2d135b76b6905dfd8ac2474f163bf011` | 384 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=16/estimates.json` | `51a68f9721fc53becc441a746d38e9f50a4c4fdc613997d2303a2ef815353fd8` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=16/sample.json` | `42361783b3a1c875b2c080e904fed0e844361cc8e1e9d79d9eb232bb14fe09c4` | 1857 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=256/benchmark.json` | `ad568809a2d79e358ecd23bc0d8c05943297adbcf79c53fb6b387fe35c9406cc` | 389 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=256/estimates.json` | `66100cbd197cbaf12e8867cc62201a3cd18206971b779859e3fea536d4af6151` | 972 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=256/sample.json` | `aac15a4af08e71ccaa1d9b5e53ab379532bd288a5845de4f2724338df2481837` | 1761 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=4096/benchmark.json` | `9db00042984a234d04314684a34ce359ea5c8d3e52d787b95b8a13908b9d5b5d` | 394 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=4096/estimates.json` | `07cceb4ee3e8cfee86b2e9ec2b9674941bba69cd6e6ed844268fa097cb6cdda1` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B2__B=4096/sample.json` | `546409d49e9d5cc733951627fbc92e34919ed20f84959ce1696031d6056c4ef8` | 1702 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=1/benchmark.json` | `724ab49007d164f68b4bb73d54e7a2d9ac5e5fb1745941a42be5105e23675463` | 379 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=1/estimates.json` | `e859daf1d14a85a19614531bbfb9b7022ceecb4da7d5980d9ee2a0bda22bd9a2` | 977 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=1/sample.json` | `d5bf2e6f977a1315757ffc78f11edaa3d08fbc5cd8ebdc8fb058b428ae94e8e9` | 1946 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=16/benchmark.json` | `68a1f20da3c3131203533ce44a2607666912d3b81e67270f391512fb41a56563` | 384 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=16/estimates.json` | `7eba8c56476c49f5c022fe331c7570196bd4444d07ec8dfefe637f6fe8d48ac3` | 977 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=16/sample.json` | `a3579b1725563ea69087a6525d7a4c50e6434b984d6e19142fa5bf4e1a031973` | 1834 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=256/benchmark.json` | `3234b475ff57e0fe7ece57bff0644079a72d6aa174a3a38d53e600722a6e07c5` | 389 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=256/estimates.json` | `2dd8aeaac58a714e2c16ff42419c6fbacd2fbe09f48d770ca4b2a9e55395f4c1` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=256/sample.json` | `f0279bbd915a5bf78e2fe7c6e0cba135dbd3d667c2b05f99c3955aaa29f2ece5` | 1753 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=4096/benchmark.json` | `1d6370b00bb198a0d8491ef76f7d7eb0a88316f2b4703b460d4b40a2016eb0e1` | 394 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=4096/estimates.json` | `0be0782bcefdd406809d110cb837d7ed8a6ffba4efe0004d53886210cb187607` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__B3__B=4096/sample.json` | `ecddb06c93447da4f321ef87b94d87ec9b2ddd82753290ecb74e47031b2574d3` | 1662 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=1/benchmark.json` | `fcd07f31b831715b4eedb899f539f1417273812260bc3f67918a94f6e0d02dd7` | 377 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=1/estimates.json` | `3c9421aa48e104d99cea87d823827d0dc3fc797951a52fe7d6390b8d099da7b8` | 995 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=1/sample.json` | `05d449b6007d5b049645c3dca0d099d19bbfea7ea71ef7425d128dc5eaca0d3d` | 2023 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=16/benchmark.json` | `bfa6e86f845e4d9bcb64fd13d5a1b4a4e0a907a8e592296b253f94b5f933975c` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=16/estimates.json` | `3de64ebd7015f1d8a6b95116ef6e3826f9607612008cf2d5f62061ff620c2f3a` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=16/sample.json` | `001f285b85853c10be488934a926bc0734f9cf578899d896db28ca37f34584b8` | 1866 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=256/benchmark.json` | `d1b1d7343ac00a2843664c487510776340979770962816984c4d3912a18e7484` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=256/estimates.json` | `c835121b845fe45348dc03809332340adfd853a950c4eeb012c16b1936abf9e4` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=256/sample.json` | `e9ae87fe7749d132b4d1475abcfedf5cfdc72f6f0214959d69b0c74d0f6a9e3a` | 1821 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=4096/benchmark.json` | `96f97efdc8bc58a5b2eb2b98a13fb9e3ba0aedb77721bc324f8aa401b2cd53b3` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=4096/estimates.json` | `cbf7a3b39b88a0a51aa5bcfdee2dee5c6ce72c74259f671899f108adb679463b` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N1__B=4096/sample.json` | `84e1a3b8b458b53b926ae347b9ca966e967189a83e5f370cdda242383f876ef2` | 1730 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=1/benchmark.json` | `e77ead20d8fa1c3039b8b2025bcb1867cc9232f5d86ad033c28b592ac80271ce` | 378 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=1/estimates.json` | `ba0387ed91e54254ca0664298bfe015b0d17dd723cceb2e8c169cee40780cf14` | 991 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=1/sample.json` | `18bd24ed60f4db584a0b00928b28ddb6e87c2936bd6d080633714a8f0c2a8fee` | 1994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=16/benchmark.json` | `bca3df3d73a4f6f3d58e6c84b2b1f06f650325e3af68038769a25c454580eccb` | 383 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=16/estimates.json` | `847445ba1d9430c569495043f6a9e563ece61769ad0db8b66cac23aef8444878` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=16/sample.json` | `276726bc96f4fad62f268730b8366d0011218a19bdc07ecac3c2f837a7f7cb63` | 1856 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=256/benchmark.json` | `2f7271a4e24af786b7717f7b44d72bed9290ac26d60a9f0496d04a3d7aee16d9` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=256/estimates.json` | `0b897c832c5c43f35067db74ddd680ab5f1d30283b3c5d81369fa1128b6b378c` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=256/sample.json` | `84cd17bcf1ff36d8e03ab4020c8f72e07843c60460b2fc23435ceac7e19dadbf` | 1791 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=4096/benchmark.json` | `3946912dc07ff17d2ff873c1d017d5246eec8349c8bb3cd576821df79a39352a` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=4096/estimates.json` | `0a7faa55259ffed6a42bed81e5b540229bf98baeee3c3b3f3bb9532e936c0a30` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N2__B=4096/sample.json` | `dfc52c7a81697e9da2115ade3b31393a9bd9e4dd87f3043952c9eff7e591a7c0` | 1693 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=1/benchmark.json` | `43be8623eab53a7e8e24a208a4063be3916f5b3033a29198b5a9813148cdd622` | 377 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=1/estimates.json` | `e023e1ac44287d95aed877b93de85221301b3eaf83be77f4300aa2737e09e4d4` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=1/sample.json` | `4ba9ff5662939e23a1df4004305ac472ead4aebb0a9c1f7c636b8fe5bafac913` | 1922 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=16/benchmark.json` | `46944cb8d6574a8b90e4aa015287707db87077d48036654babd5ddbe52ca6730` | 382 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=16/estimates.json` | `72d7cbd6e0680c2c4f3d3f6d1d30fb3f54cdbba1934bc71950690cdcc0d5cde8` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=16/sample.json` | `05b87e929102850ae892688a7e1a04245c4977107ff6daac4e2ec3f2c0509dec` | 1779 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=256/benchmark.json` | `3b76462d13a0629c71009ebc92361ebeb14fbef8582f0ba4d1feb29f76b631a1` | 388 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=256/estimates.json` | `802919214931fe2118d217cefd996ecdb52cf4cb54d8812d93c2959411f5af08` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=256/sample.json` | `964937ff9ba1c30c04af494c87750012cae036a6abd39e4781e99b13adc59f1d` | 1721 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=4096/benchmark.json` | `e0779d7c441c8fb15f95c2ccbe84267ad7e09a38153e59fc2f4ceafd18f6d99d` | 393 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=4096/estimates.json` | `271ae4f217e870ef3d3aa242f9cc0bfadca4fc18b8b697aa1a25d431fbe9d923` | 782 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N3__B=4096/sample.json` | `367080e2611b5a9be16c82a7030cb95cf3cc93884369a710652f6597094c4d04` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=1/benchmark.json` | `85b56d21d089288b194afc376de2d30968e012784abe0122432b1a18fbc920a0` | 379 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=1/estimates.json` | `507222d44374b739943480a07fc3e2b139cf33f24efd3b5f6e6cfd4a250c98d0` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=1/sample.json` | `f26bebd9f689f8a5fd28617befa2d65c122d9b4aac7bfdded90419b1a8c62d41` | 1751 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=16/benchmark.json` | `b035b19f079637feb1fabcd58f8ed18de42f6a52cf8a1e47384d14dee229a651` | 384 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=16/estimates.json` | `75518e6a21c54aef7a8205f763b3e02be4273560a07dc26a64fe1bd877462130` | 794 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=16/sample.json` | `b3581020fccb817b7d161aebde44b3bf8aff01bfa35c587dfd8053f731cda4c6` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=256/benchmark.json` | `3242399ac2de1f5df812d054031ada485e498d231272315ced9303d012c81788` | 389 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=256/estimates.json` | `49358b0298fefe406a0c2448a09047dff74a2c9abb37c7422fe1d9551a96b375` | 769 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=256/sample.json` | `4bc18a677e00ceeec5935b971b27e4634f248027dcf188a6a255584726f60082` | 1644 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=4096/benchmark.json` | `5adba2c939def88f4a539d5a5ee66c92b6b846b7c3fae0d175bbbe0378cb37db` | 394 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=4096/estimates.json` | `3dfbf8a7ba53c455556589416505871dd27fdb7099218f987fd21471cde01431` | 779 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__N4__B=4096/sample.json` | `e9fe0fb02700796c1638a74321ab65db3eb96740ab7e08cd41f542b4ec68e3fe` | 1744 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=1/benchmark.json` | `735a644434890a667268f629335d3640326724fef292346d4e98db22da9399d7` | 413 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=1/estimates.json` | `1e587930d9c2f377138f6a57f10b301311b6e3aab881150fc48cc77506b25a78` | 796 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=1/sample.json` | `446d0aa1d3bcfd2585184add10d98fdf841f631f407a8f28801b10b29ab22bd7` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=16/benchmark.json` | `91888de201f5cc73eac8c523ce600d94a314fc9607668d69c4f31a33058364a4` | 419 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=16/estimates.json` | `4118a64396a5dded0d98288eaff53e1efafd61743fa6f5d9de20e4ee75e64f85` | 788 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=16/sample.json` | `f607edc69d7dc32534f4c85707cc47b4c185c4232528003e494823e565903daf` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=256/benchmark.json` | `a8bb1d91b4943d700f3c7e1657a1615676e823fe3c3e824d1c20802726916114` | 424 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=256/estimates.json` | `b3b67a4d2f45a863cd2e3f7b0cadcab89b22ea16935b45660102e9d19305c366` | 797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=256/sample.json` | `7e0d8d3d8e8db661c83ad02def1032279c59c4c9e4d3f50c6c8809db9e59e9d5` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=4096/benchmark.json` | `0c0d9f81d957038588becaea4d85e603f21e2d8b9436673afb8c7af3c4efda59` | 429 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=4096/estimates.json` | `344f21dd09f1f8cca3743fd94c01e8120a8a1c197334aa61d3a9737d2e21fa14` | 769 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2N-mother__B=4096/sample.json` | `ce20e1d3d5a9d4ee2204095094748e25bac0873417b33c902b82a141957e3451` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=1/benchmark.json` | `aa7dbcea44fce8f7e73ce10ab9733294815e6a7c6ef163ca080ce893ea0020d7` | 413 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=1/estimates.json` | `d45c12040957a859195afc1060c1ba201f5fc81770eb4cee9f754198552c7ee7` | 800 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=1/sample.json` | `f6c159aa07108ca0ff829f55d92936434c7603e13b2db38f03aa897430eb16ad` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=16/benchmark.json` | `d1c299ddec4e7d2e26b2c4b21b51110101451700582e5ad8b7fb9caef5e639ea` | 418 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=16/estimates.json` | `ff1d088567a570f1131d565a94d95d80518a5f6770ebc8efd7608a023684fe05` | 792 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=16/sample.json` | `60204030abc07b77498b8afcd133c4e2f6fa68009351e372a0b2e46dd92e954c` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=256/benchmark.json` | `ff976c4012b194e8114134f75a0dbc04137a28e44589a007823f883a15864536` | 423 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=256/estimates.json` | `2176fd5bd9134b9e6001380e0d421b904b94ddd603dbbadf67c52eb6cb699401` | 794 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=256/sample.json` | `8f362dcfb82a640b9369241d8c19d720dc6d806ddc5c0222dcafaa19ca6f182b` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=4096/benchmark.json` | `487ad123801b7ce45f1aad834385d0f72c267b34597787158f38fa22a2db3a03` | 428 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=4096/estimates.json` | `9ee4d49b049c1a900c83087dd1fe5013dd3da39c960acd59b179a4035122b6eb` | 781 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_encode_w1__selected=poly-remainder-scalar__W6__warm-reuse__T2S-mother__B=4096/sample.json` | `4dd04a125f2cc172f9e371003c54d8255c67aec32ea87e1890e166e86d6c3425` | 204 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B1/benchmark.json` | `fd70bdea5d53eaef89c1d716d44a048a9584e4a296a1cc1e8f45e9fee601ea43` | 290 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B1/estimates.json` | `8608e3ecc0220f0367651bc1e7ec0c402bc52d7582a1e8c0de0ee067f2b10d5d` | 996 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B1/sample.json` | `77514374198d7b359dd9d82bca4b0a3fbd0e419fae0dfd796534680b4ca66974` | 2086 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B2/benchmark.json` | `fde17562ca786fac96339ae57af22101adb16817d1d15aacbacdb5d8a44a91e0` | 292 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B2/estimates.json` | `024e89191d3191b4f20b579a92335cd9aee2947322c159d6ce728a479e386ef7` | 994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B2/sample.json` | `9d7fe98e7d5244503b984480328b6fd6e16d0d293b3e1257d49f8d6704ff4eda` | 1982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B3/benchmark.json` | `74a482994155b1f69fb2726e54114cbf2b00337cfd9cc5c219f8cac906d11f58` | 293 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B3/estimates.json` | `1f7c5f1e96889715915b7036d57bf8c817f7468c68eb44fb0c952e7acdfc2c7e` | 984 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__B3/sample.json` | `4c861e3e6f89109692e94601c2defd24a4987effcb229454a2f11d9f323bbb80` | 1919 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N1/benchmark.json` | `3ee361bcb22658d5b67535f5b592562424bd3bf3ccbb42a41419300be16dfa5a` | 290 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N1/estimates.json` | `e747b6ad72ba8aa60690216f44547b5c068b4176e18035d44118121e8c1f2892` | 986 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N1/sample.json` | `bc768e9cb4b905b6073e04d07b8a7d080fb728c5eddfffe2b85d3ff3269f4332` | 2045 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N2/benchmark.json` | `6735d4281cb31a5c82271c290a3fd869847d39ce62bbe134922c733d2be8400f` | 291 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N2/estimates.json` | `9d2949649326151b5464d9b2cbadeee57393744ef8aa00a34b7ea3c34ad9844e` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N2/sample.json` | `732e42ae6ceb810f194714cd838344e95ea0b9925c8e2de2727a6dd69c4518d1` | 1971 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N3/benchmark.json` | `5efdf60c9aa3836f635c91901b0a8d6b4058bf3dda1c8f8975abcc38854d4408` | 290 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N3/estimates.json` | `2f2c43cecc8b4b22c7fc6c107cad1c4e278024cd18d149717595847d2a5b2e41` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N3/sample.json` | `e60cbcfb75fcda8dfc8fa94eb07013762ab2da13267d6eb5c5990872b9ba2441` | 1897 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N4/benchmark.json` | `a5f2a4431d6557b78d5b89abc46a5b883b2d274f80f41422fff34c484b7dd733` | 293 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N4/estimates.json` | `026dab73602235904f4afaa68cff172ee7ea47589409fa31a7c26de4fe540766` | 974 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__N4/sample.json` | `46afd4af033a4e8c6aaabbcd43936dda25955949b4d3de331763504e03faed0b` | 1698 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2N/benchmark.json` | `a9b9de39414c8de5a512cbc20cca2466b2d1761503468559233b737c49d0023a` | 302 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2N/estimates.json` | `a041c9d46b9b8a6c942cdf36b26742f2059281f8841815da036216a49d3ec1ba` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2N/sample.json` | `d89a3f70683ec01f74d5dcc8582a1a8c2252680e7049774bfa9d7cf59bb77401` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2N-mother/benchmark.json` | `ab861cd06d91e05dfac1779f4ff24eb40c08e638e019153c6402e1cec12d9a93` | 330 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2N-mother/estimates.json` | `df08b20b015b8b58fa40a0156fb9ffdd8a179095653f5117ed1875276b07e6c0` | 971 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2N-mother/sample.json` | `2635cbacc888edf2749f3c59e01d99b75fb8bef79ddc9a72e96175aaeaecec01` | 211 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2S/benchmark.json` | `eedde02a0600a74000250aa39c56b4bbd7d3db5d9f425e4f918faf5352f844db` | 300 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2S/estimates.json` | `7f52deff11281044f315cf56340a1dae278891191714f9905704df6a9b6818c3` | 781 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2S/sample.json` | `5b3b0aaca06f1510194f4031d189ef503dd7bc0a93754b20d08faae9d3725292` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2S-mother/benchmark.json` | `48ae47978fd784933a6b9dacdfcc2c7d0999c112243b358373498106dd39e9cd` | 329 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2S-mother/estimates.json` | `0cce60081760b24fcbaa24fa159c51a866f7ccbc08558c5045c94f60477a5fe8` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__fresh-alloc__T2S-mother/sample.json` | `8308766bd0f05953140a9dcc301317fb2589d7c5c8d014b20ba0c921f6b20f3d` | 223 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B1/benchmark.json` | `d71e131aa78c27b4703d3bd8517fcb7e32336e582839737a366435e3d69bc5d1` | 286 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B1/estimates.json` | `e79bf600766e5b61221633df59fb653ad521aa979a2d55bcb9af84536f865c09` | 997 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B1/sample.json` | `e9b405545e402eb323d96489a4d7c2c060b06088857631d770fd309640cacdf7` | 2096 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B2/benchmark.json` | `928dd6e217a0f2a79c7f74a2c0b3611819ee34ce84a2e0f7ff569d96041476f3` | 288 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B2/estimates.json` | `843b019a3faacce121b95d29181db19bf726c600fb28cffd5abf949d71d11241` | 991 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B2/sample.json` | `b87e8e29210c7c2c294065a622be47bdd4eb0356c91161cc5f3683b8e73a3aa6` | 1983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B3/benchmark.json` | `98bc53071ba159092d8f23afbd3d9ed195288bccf12726ee85345803f70ab178` | 289 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B3/estimates.json` | `8fd3390fe36ea1ba5a321fb976c78848d0f74738df0a5e6c5723eba943fbe762` | 988 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__B3/sample.json` | `f79e48c169222ddd446665bce674e21fc109ebe7a3da0596a327288611e9e0c0` | 1920 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N1/benchmark.json` | `1b376e6585a742744c9a4840470361d28161dd0aee16101ae210aaeee0487428` | 286 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N1/estimates.json` | `b754438858b0bef541541d3a75eaf6de309de21c03cf652b5fcf8aae6b0763d3` | 990 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N1/sample.json` | `ece1215445225c65b8b4d39a75c345b71b8fdd64878885f8bb9aa22f497dbce5` | 2056 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N2/benchmark.json` | `183f892407d55a38b9626e5bc5be5d82dd89b1dc42477771b2f669c13f46b8e3` | 287 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N2/estimates.json` | `c9e2e45e1a503a67f92f31e40925f803188f4eda42cc9378b7794feebdd1805e` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N2/sample.json` | `35fe98f7bb3348c6dbf201f069b56d150923af354fc008632bd2c560ea961d11` | 1983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N3/benchmark.json` | `1e6bffc7143e351cda5ccc2c3b2fab80611f1dda9e60f5abf0b95ee0285ff604` | 286 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N3/estimates.json` | `fb00802a29116d8cb787fa409c913325b377793144daaa02a24854e0ad569ecf` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N3/sample.json` | `5abfc5ba49b7963e17569cdfc5cf4a14482328e74ae83131c3da84126448ac94` | 1915 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N4/benchmark.json` | `216bde590af0bc7d6301d7c2be5bdd293c5b76f5240a600f3863318a71952038` | 289 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N4/estimates.json` | `b95dab8f4129b38e7b4b39444133ad9fc15da31b4f2a6c7c405f73c78eba1f95` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__N4/sample.json` | `edda15552cad9522c5b3f7d3c9b482a7ba1ef568cdbeca0af44cbfd8298def59` | 1705 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2N/benchmark.json` | `6aaedb2d3311ec07a403294219012cf4696a6b482bb5ddfa24439a60c1064155` | 298 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2N/estimates.json` | `ceaaff7c8b77c54ba32b632bd676e9d159a498f73156421bf8fc615e6be21bea` | 775 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2N/sample.json` | `2679d36983c3473ebe29115bc0cff336088bfb09fe6bc669ce6537d153e05f3a` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2N-mother/benchmark.json` | `83a614f5086f62401c3f3eddd8b2d5ac7d27055b70c111712206c2f836e635a0` | 326 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2N-mother/estimates.json` | `ba1f0a0f665dbaabdd6b56375eb15cd58c007bfdfad43746b16f12608467aed8` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2N-mother/sample.json` | `61cbbef2886a75e4ff7c340f09dcd5f7c72dbf01772d4ede25b7090588938b22` | 215 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2S/benchmark.json` | `d39cd22532ec2979da14450a4d9814f6fa18dcfd8e9d21ceff335a507f1cd304` | 296 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2S/estimates.json` | `6dc8b21cb5557f5a1fe41e4b243a470188877c64de92cbce643c6f2911628200` | 770 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2S/sample.json` | `0a956af5538a3e814f66f5bae631ad67bb835f32cd9ca672ca2cd4686cdb5c23` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2S-mother/benchmark.json` | `3dbe68721520a7b6dbab055d2924316c82632d4afddbbd0936316862bc57ed03` | 325 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2S-mother/estimates.json` | `a51adf822ca30c7f88b37f728542f9e5811932b80b5fea7046ffdf6340e7df92` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__materialize__warm-reuse__T2S-mother/sample.json` | `c098977df44a02c0bf97396845320c956b34b2ffa42ae6c42b3bf75a1970748d` | 227 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B1/benchmark.json` | `486cdcbd980c6876abf1644ee5b0e325395553ab0e4918bfc537a4bc56fa24b1` | 282 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B1/estimates.json` | `1ab271eb04861644e29d974b0312e4d2f89c95a3bfb3ea40d8707d0795f91bee` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B1/sample.json` | `582c87e0a3b9a4545e2cc047957c5c08978489de0555a2679588497cc59bd4ed` | 1989 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B2/benchmark.json` | `ddb92ad3c687b685771890145341e3a1d7651a5bc3deab2b751301fd982a0b41` | 284 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B2/estimates.json` | `07a02031db2c163e4d50802d69aa78e5595a5dbfcd220426a9210246cce9a29f` | 973 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B2/sample.json` | `2d26ae7661f9cba94be90a40b34f22063f33d44c9e995f525f0762a5457d67d0` | 1790 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B3/benchmark.json` | `7a4cfa2a85b9f209d6303ba370d7c29864488cf3a056f8b05c56ee5a3d20040a` | 285 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B3/estimates.json` | `9881cfa70691f374b942d4968253993195f935343056ad46692cc9ac61fd3620` | 965 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__B3/sample.json` | `b6677bd725b65c1a5d69bb23553070e58fb06d3ee53ffefd36ded24ca4795164` | 1731 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N1/benchmark.json` | `8e8fe57e5d5cb94910216eafcabab851b0d6ff346f342a6cefad391cff98a962` | 282 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N1/estimates.json` | `eea043f903bbd15bd1ff2cdd34a00ba19bcf0a2bb27cdcf737fb63edb246ad9c` | 989 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N1/sample.json` | `8b9a27198514842fd6ae2f1e7e76c6fbb6078e81523fb8e6119c333486fa18d0` | 1997 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N2/benchmark.json` | `a4ca7ba85652c95d92688070824f8876e33c3540a926487f0985c1cd8c0ddd40` | 283 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N2/estimates.json` | `ab62a052cb8aa8e5cd5c8613b971d5ea31f56bf3632db98e69de76866f58f5b0` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N2/sample.json` | `f708f704075f9180b982374ca301a6061b9c5d8ad319508c769c1567057a4dc2` | 1862 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N3/benchmark.json` | `100f638d1be192e5edd6a60cd639feb709e4e5c8df3318b7df04b6d79feb419b` | 282 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N3/estimates.json` | `455664d963e26bbcd5f936929bb906af1f018e2dc19c6d1e18530e46f987482e` | 987 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N3/sample.json` | `3d272f49ed6ad299df15d6a823a5dc6d6e1833ef07ebe83e25c0be92bc3a4d00` | 1818 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N4/benchmark.json` | `adf6dafcacb3b9c79c45917b7a4c60f4d33791ff568117340d9bc68fb4e4b368` | 285 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N4/estimates.json` | `a535f801b847db9eb15f2efd4e6dfab58ba1eb3b802c1281739930f3fdc7878e` | 773 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__N4/sample.json` | `25bfae555ab2d40641fa6ad571abd84cde42c203430d6207d0b94923aa188053` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__T2N-mother/benchmark.json` | `39140da7b0e26d5a529741b86ccd6c5867e193e8b65725f6f033b4b32a50d7d5` | 322 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__T2N-mother/estimates.json` | `6798a2309f76c68c3385fcd2e81f6edee3bc029205940b01a65b888a55e36872` | 773 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__T2N-mother/sample.json` | `8f7e9f9951cbd925c51e14c1f5f88fda888d6d969105e7f8d81947c113425b51` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__T2S-mother/benchmark.json` | `29d2e3bcd782f420ec42b124f4878285fe06104c021a40114f01a24226bbde46` | 321 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__T2S-mother/estimates.json` | `6dddc2443991478c3f881a1ec56a44a972b5d01b9996ecc449ea6eb51c3f146f` | 768 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__fresh-alloc__T2S-mother/sample.json` | `434ff8833912b05382f97a4b38464c4b2299019152e698ac6f71ea0f2f28adb0` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B1/benchmark.json` | `31df05753f449773eed65c3dcc9a67479a4bffab098330555a4c0aa96817334a` | 278 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B1/estimates.json` | `18a5e848a0150feb138b0d0131e222e243f9edffd5d88467b5284fc81c1d68c2` | 988 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B1/sample.json` | `84cd44f090d18759fac84b5a81e67dc218fd74ff93a1808d40b76db864af192d` | 1987 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B2/benchmark.json` | `b96df24468b20131ee5aef92fd5236176fb4b386bd68d765aaed69b29450c555` | 280 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B2/estimates.json` | `673ff920864f9e4c2b9d567906c95778361f8f60aa794ddc41a405c7bf0e125c` | 960 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B2/sample.json` | `40052385d697f269b4d0fcc4e132ec1c690e89caa223eed512d4986a8b29ef2f` | 1791 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B3/benchmark.json` | `ab88126c9c078d45da7bea5790d42e8059e72f233acb105d77b16daf76072d57` | 281 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B3/estimates.json` | `3125b1dc6a8f82e08cb6233cd7b1cdbe9c40743f3b9db07e551b912ac33bee37` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__B3/sample.json` | `583d784aabcf90daa4d5df0a9a5600c55b885644f0b641d623e8ebd94b7096f3` | 1727 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N1/benchmark.json` | `387d7cdb63a8ba81e3086243d6e1d40e49cae00844c3bcd1c27c39b30414c96d` | 278 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N1/estimates.json` | `d83e7f23dc149ff506be6472ee245731bac36091c92e2b763c30bf2be25b615e` | 987 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N1/sample.json` | `47e1c873a48b3f1c0ba00258c48972fa9c9d650309b60a254acd3ea519e584ca` | 1998 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N2/benchmark.json` | `7765e8acf33adff30f77509344f2d626e20a54109126481ede0ef51c7705b5b2` | 279 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N2/estimates.json` | `a75523dbcb20a2d6c0da277e0e32a67dd52d3c722a49bf7709b9bf6ef88e9a5b` | 977 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N2/sample.json` | `03bbc039c626f27f95430b1e1420c407ebf62301ec95c4ef3af711b72e077ad7` | 1863 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N3/benchmark.json` | `8320f3433b45e012791b9a80877a493a7388147319ee006c5e82c34b4452c4f7` | 278 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N3/estimates.json` | `f3dc4b85018a580353718fb95941118bfbeb4d5c62e8722b550ce49495d339c1` | 968 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N3/sample.json` | `9324bf0c74ca1244db67db75c68e1f15a3d6116ff8ef4bad72f6d5ee3fed0e89` | 1818 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N4/benchmark.json` | `61e164eb9ee21e4ccfd83b700ec12fcbda50478bc51efd6ade2b057ce229785b` | 281 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N4/estimates.json` | `dcf16fbe829990876b3c6de0f734edd0d0b1472a41cd7a76b961307fd26a0b3d` | 769 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__N4/sample.json` | `c9e3d287f8a371d273f7870fabd3e2efab38be8dcae530d823b005fb58cd7bef` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__T2N-mother/benchmark.json` | `28eab5705cce40dcd2e4c0d2909de444125abdddecb2446793d633b893bbdbb4` | 318 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__T2N-mother/estimates.json` | `5851c05e568c4e51badbffb7da7882a8466585bb4382238664247d8dbd42fb74` | 772 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__T2N-mother/sample.json` | `3605dacda77bc5eb5bf9dc48967a01d23392d52c1905a68ffe741502a3bf990e` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__T2S-mother/benchmark.json` | `57c6378f8e67743efebcea89624478e3fbf8ed7b6c995a8a9785be7f27451ad5` | 317 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__T2S-mother/estimates.json` | `7b5d031626718dd82e839e0c5a9c6282599582b35d7bdae0274a05d2b5d1a711` | 768 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_genmatrix_w2__reference__warm-reuse__T2S-mother/sample.json` | `ec72a686183442023aa5482018e059f1add87a7b0645293d4e24033b493c87ea` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B1/benchmark.json` | `0f6ddbfd88a4f6c9101232f62fe07a6bebcf098270e8365082f9e57ac26025f8` | 283 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B1/estimates.json` | `cfd13dd6dda72e99f44113ba9f4c978fbaa0ce849e22cb2f1136d58fe2db5429` | 993 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B1/sample.json` | `5ada8f113fef5a76fefeed989372a9b94a3d4dcea41fb439f66bbb3f1634a2c1` | 2035 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B2/benchmark.json` | `ada2da0401f3113f4575446fd4f9bfd839353e78288a679d41152a029980951c` | 284 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B2/estimates.json` | `41d1d320a5e8569f2e18d577ebb34fbe035410a086437eec1fb6fdaf4d890f0b` | 979 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B2/sample.json` | `3e6838715a6f00dd5a155466da343b0d722bae4baae5e4d129b2810742d163bc` | 1874 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B3/benchmark.json` | `0e041448d23e52000a9f58c23dea9adebbd1dab946950a1207d80eb2d5dcd03d` | 284 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B3/estimates.json` | `f2ceb4d6010bc4c0f35fcbb68a825787caa508509507042d87e473a5c9bb189b` | 987 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__B3/sample.json` | `e018fa4e765938fe77ddc594b4a7d2c376a673f78929decb8bb292a7be653b24` | 1824 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N1/benchmark.json` | `0d2ffa357be0ee38606936bca76c819f73af7175d8506456d80955162034ca72` | 283 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N1/estimates.json` | `e3b7221216e0bb92d2ed956bb1b090fd36d619f51e63db6d0230a8cdf2b15698` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N1/sample.json` | `7d9446612eb1807ca84ad0f9e2675d6f03eeaaec3dd4426756e5e0bbb869d40e` | 2060 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N2/benchmark.json` | `490964236d248360c30dabe4c636cb5434bff208d743e00d090f22917015587b` | 283 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N2/estimates.json` | `477190fa5635a2f958074b51938325a5ab1186b751611196b57a65ea820f5cce` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N2/sample.json` | `c6a3b0250e05b369746fe2799d47ca0701be7117dcf4a34ec83c5fb59b82351d` | 1999 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N3/benchmark.json` | `5a326d1825cdb53226486579d214202e53afb2a323456409bb6db2ed6ccbac8c` | 282 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N3/estimates.json` | `0797b41bfc57d0626b1eee2cddd5d99a0af9d006fcffe29f045c90af40e8c0ca` | 985 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N3/sample.json` | `94cfd5c255838aa5ed98ed40a13a6fa27270a1a9ea36a5e8b49c51483f2063b9` | 1917 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N4/benchmark.json` | `e2dda6c847af4441f8410e9658995918fc781d401967c52072eda813f4389893` | 284 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N4/estimates.json` | `8b19602bf71db8e2e10301b190eabbec862d4af5c0f3d106dad0eb088b81c077` | 981 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__N4/sample.json` | `84824360ec7e5fa92e06cf824a6f73babe99819db7727792d65afe90456faf4a` | 1724 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__T2N-mother/benchmark.json` | `7cad97155f7bc811f7344221b820d7289290bb8201409756e78ba861ddc9723a` | 320 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__T2N-mother/estimates.json` | `e5fb5058c91181a5b0cfdae607a6f1da69d821eccedc93e1ece12fd2314ca420` | 965 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__T2N-mother/sample.json` | `8c7b8092cd6af4e98901f8b5c47df24209e74d2b9251af75902e2a0cc3aea00b` | 216 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__T2S-mother/benchmark.json` | `fb86a8fbbe82cc4013e5512160cafa5a400f266c8afe43a258fb0c50f49ccee3` | 319 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__T2S-mother/estimates.json` | `6619c102bf912a4eb76e5fc258c9279dcf0ba9fe5c5eb3bc49b5ef0e0f6f599b` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__materialize__warm-reuse__T2S-mother/sample.json` | `28de345896f65c09081475f3fcadff7a2cbd23749efa83e35b25031cadaee2e1` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B1/benchmark.json` | `d29246b32f7ae632f1e527787badfca74331c591186245fa4ab44333c7a3695b` | 275 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B1/estimates.json` | `153a930b97a49e1867bd9490951e257f492ded60eb205dd58821d658f4e718f2` | 983 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B1/sample.json` | `3561778e5185251696ed516c54a1d71ad6431fe877326b475612c24fe59e2591` | 1970 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B2/benchmark.json` | `f489dac42afd23b6a7db773465ddcd7b6fa5d4bc987ba7966fd99f14fad359fd` | 276 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B2/estimates.json` | `089adcb29b022b5d0bc1ec96d33a188bea704118f76129aaec0b50ed428cb491` | 980 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B2/sample.json` | `9368654bd6271338db3dd7580760cdef65dcfab2ef12d57ec7dc38b0470988a5` | 1797 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B3/benchmark.json` | `43479bf3e2ebda49a0b37a2215134e5e1d574b48dfb7fc7c82347882f9659c25` | 276 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B3/estimates.json` | `33e1730dfc8dae63d7036c851f76f8508c858cc4b398c9f8fc92d7d62e2e79b9` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__B3/sample.json` | `1d7cf78e58265793899f31a122e5ba045c47825a3b62b52d6a4cc2d56dcd533c` | 1724 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N1/benchmark.json` | `c8ce5cf7614b3d29827ec2811013141603b5b9d2ab16b41a6ac4967652124a72` | 275 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N1/estimates.json` | `3227a16a99e15cd7fdb53e4edceb88678f4fd9e31a62a186fa31a15570d68e20` | 992 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N1/sample.json` | `f176eaefaf6b4aad3cb796e15f8e7e81e7804a923c368838e1811f8890c09834` | 1994 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N2/benchmark.json` | `74cc76fb65f01a48f9e316ef7789705b0925e98605941f7a88bc4b3e0c3ce9e7` | 275 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N2/estimates.json` | `3e457bfe3c19f3001ec7404eb8081485ba706f799fa284504c37a76f2a4a1342` | 978 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N2/sample.json` | `d9113bcd9ecffd59e3cd467ed7723ac7611f18ca5c88cceb57eee1ba50354a62` | 1863 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N3/benchmark.json` | `96ea4a10d3593d66ebced0b69648303103324148a93c061fcae9586fc498c8f2` | 274 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N3/estimates.json` | `b15964c58c7a2d1febbe21458d277784134477820ee25f2b7cf2f4199df829bb` | 982 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N3/sample.json` | `2d8ee9d2ae16c17addc035251d8f8d4f5fa0d309a68df290dde9480d0acae11f` | 1819 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N4/benchmark.json` | `d5281c337baebcba86e070e1e4e3f586c300c643a51a0ed7fb2eedce95d41b29` | 276 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N4/estimates.json` | `cb2e4b2d413d9639aee95be4ad0cc97f14f2152782278ce31142ccbbe598def5` | 786 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__N4/sample.json` | `b16b3d20adc72e9bd823761fd9a38737e092529ead36d3333f806b272256f86f` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__T2N-mother/benchmark.json` | `523cb03fcb8aca8b120bfa69980bd3b9125ab9645c4157772d52e0ff34151581` | 312 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__T2N-mother/estimates.json` | `c9589a6cc65b35500d64bcc776f63f7985bf061316e4536acf72172a84daf77c` | 776 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__T2N-mother/sample.json` | `72b8b830dc94cb91e36208aa4b86167dc8ce1309514113d0222bcf0e9b43d42d` | 224 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__T2S-mother/benchmark.json` | `dbc6751433d1a4d16f0f0f3d2d8b8e7a93eceb0f58685ccf82ec0bcfa0ad42d2` | 311 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__T2S-mother/estimates.json` | `7569ba99422f930f594e7a38bbf3d4136264860ff867848e2d9b3fa9b5eeb31d` | 772 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_paritycheck__reference__warm-reuse__T2S-mother/sample.json` | `b4748ace2723c54c766238bdf695e2e87dda0bf2e53bb4167134e3f9f84984d9` | 214 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_sequential_vs_batch__batch_operation/benchmark.json` | `13408d3cf1c161c234cbc37e270974b174c91e00f08a62176863fd1eaafeb9cc` | 275 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_sequential_vs_batch__batch_operation/estimates.json` | `b920444f898a5b94559ed0159eaf7b792d06332ddddd878789c537556cda3b91` | 976 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_sequential_vs_batch__batch_operation/sample.json` | `13470a2cbc1ab1f2c543a12fbb437656676d827a514243a34757f184b1df3d42` | 1865 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_sequential_vs_batch__sequential_loop/benchmark.json` | `d00db277423e73f8fcac65ace03c122da13364437d4569ed9e894fe215a56b34` | 275 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_sequential_vs_batch__sequential_loop/estimates.json` | `ea1f4a32365aa1116841545f59726b1016e70c202263b39d8f09c61e7a0d4da9` | 975 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_sequential_vs_batch__sequential_loop/sample.json` | `d526bdd049dad92bf866185b93cd72cffa88f3297e121c2f33531de72674a346` | 1836 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_single_vs_batch__decode_into_loop/benchmark.json` | `e2d29156735ed9b371972e15ab61291519e900b23854948cea0af338750ae18e` | 254 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_single_vs_batch__decode_into_loop/estimates.json` | `e7fa55d090ac7340599b03c84e1c27fcb6568d28e12c4661c07fc249d1c21e26` | 763 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_single_vs_batch__decode_into_loop/sample.json` | `8b5a7f82776c6352f7c2f92915bef3ea47245741932091ac454b5f099b989778` | 1544 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_single_vs_batch__single_loop/benchmark.json` | `685e5933ea4767dc8644c77cb03d5973f6312a204b7ae9da0563dc7410d32649` | 234 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_single_vs_batch__single_loop/estimates.json` | `c4c3e19a7688eb793d15b6e5c7464245d99c40e792029c55e49577e996c72a9a` | 756 |
| `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2/samples/bch_single_vs_batch__single_loop/sample.json` | `31c4310da3457d52ef4c40ed53f76d718d327744e08d2d382cc9c3599b92bac6` | 1544 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_1/benchmark.json` | `9845e3a0638c5254e4de86192737de2436fb7347d3738297f0f2072e5056625d` | 192 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_1/estimates.json` | `bf546a287e950c7780f9677efa9d1ec44a656be04248936d9d9bb5b2c929d9b4` | 979 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_1/sample.json` | `e1b279c4bd22183bd2e79ab3a11dbb6698e5e5ff9967bf0c07d09143f95000b5` | 1799 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_10/benchmark.json` | `8d7974570c8bf3ec1bfe2eb4694762727cfc50c1e2bd91f246351a4efab383b4` | 197 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_10/estimates.json` | `fe6d2366f4b7048ca248791ea7d74f7f643626cbc7b92a6c2e6e0149f6060493` | 792 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_10/sample.json` | `1842e6c9a0bb081d197d3908fcc5fb04f0befa2118468a2967b13b20eb0246de` | 1544 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_50/benchmark.json` | `0a375b2bd004edd8a610bbf037b832be0eb3a8d2ea6b70f63bb6a53977542712` | 197 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_50/estimates.json` | `615136efa8386255c8463edbe57a4fc14cb92aff40ef9f1b5d89d342c4e7aad4` | 767 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_50/sample.json` | `d0fb50cc809c05280edc6442760b834683d0fe9ab5e193466e30a3c988898599` | 1644 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_100/benchmark.json` | `9f464a1712b23db6d660984ab13738f8ba8de013e6ecd784b7aab32bc39aa703` | 202 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_100/estimates.json` | `e2da865ff0caa311c82190efc34c3e34e719ddac786445ae81ae0592020f7cd8` | 769 |
| `dev/bench_results/88ca7d2f/samples/bch_batch_decode_100/sample.json` | `4bc3f6df81949102912a1340cd59aee052fe1767f380a710d811c451151e9ad7` | 1644 |
| `dev/bench_results/88ca7d2f/samples/bch_single_vs_batch_single_loop/benchmark.json` | `685e5933ea4767dc8644c77cb03d5973f6312a204b7ae9da0563dc7410d32649` | 234 |
| `dev/bench_results/88ca7d2f/samples/bch_single_vs_batch_single_loop/estimates.json` | `a3c6d714b1ee39077916d5a9fd214a9f4e7a05c692b45d07e82560e08342807d` | 773 |
| `dev/bench_results/88ca7d2f/samples/bch_single_vs_batch_single_loop/sample.json` | `c52c0e8f4c4112dce10f85d855ef73ccb721df85c18d6e28e6e15dc22edbfcbe` | 1644 |
| `dev/bench_results/88ca7d2f/samples/bch_sequential_vs_batch_sequential_loop/benchmark.json` | `d00db277423e73f8fcac65ace03c122da13364437d4569ed9e894fe215a56b34` | 275 |
| `dev/bench_results/88ca7d2f/samples/bch_sequential_vs_batch_sequential_loop/estimates.json` | `2e614dc7c50360144e732ed15cfc018ebe25f912b94989eb9c41003a5a21db4f` | 987 |
| `dev/bench_results/88ca7d2f/samples/bch_sequential_vs_batch_sequential_loop/sample.json` | `d81734e6b55b52e82417b3fb3fd519ff10dc640b1a6fc7473b7b8d99fb21cbf4` | 1737 |
| `dev/bench_results/88ca7d2f/samples/bch_sequential_vs_batch_batch_operation/benchmark.json` | `13408d3cf1c161c234cbc37e270974b174c91e00f08a62176863fd1eaafeb9cc` | 275 |
| `dev/bench_results/88ca7d2f/samples/bch_sequential_vs_batch_batch_operation/estimates.json` | `01f24eab33d22d6bc6e2b605a2208f702894fc9d2ff3dc144db07cee2f23d932` | 985 |
| `dev/bench_results/88ca7d2f/samples/bch_sequential_vs_batch_batch_operation/sample.json` | `b68634203a98f03ff927590c39deaa5a9d2c3a98f92dbd04ab3783fc381a8469` | 1737 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-small-aff3ct.csv` | `78997130a40ef02b52f244fb4e23dcf421eacaebe80c1af377037a63b57c6045` | 18990 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-small-bchlib.csv` | `62e962aca5491a42d3fa313a5297348c4c01d46f3e2fa02757cd9e627048ddbd` | 5046 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-small-gf2.csv` | `114480a34021422501256152222fba7de373b471ea9e3f936fab6a3ed6689d40` | 27312 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-small-itpp.csv` | `b7a0fca5f3692e3d47d204b52227c2c8baa3ca60fc9c758309021359cd93bc6b` | 8175 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-small-m4ri.csv` | `e489581831e20c8ebeba10bdc6fa18f9e000ded3af03f2807c00ad4cfb1f25d8` | 9403 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-t2n-aff3ct.csv` | `0c2f937df8c69be5effb382abeb5f8aa9cb551b1041abc0248c483fa4fa857b1` | 4962 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-t2n-bchlib.csv` | `b1cd28210a86d6a56a4c31ba8c56628168f584385588cf684d1177318c35089c` | 2708 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-t2n-gf2.csv` | `c84e385bfcb94111d01774f818bf1fd3d2fd966b6309313a94aeed7fef13ba4e` | 5468 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-t2n-itpp.csv` | `eebab57153ce9b424ddc0e01d3dd602fbe33fb8b25912fbecb06c48114d21cd6` | 90 |
| `dev/bench_results/4e732b56/2026-09-01-4e732b56-t2n-m4ri.csv` | `9a23de2050cc37f19ac3fc95f31fcad4c976f6eabd92107ad14ff8957aa4f898` | 2683 |

## Verdict

REQ-01 checks (non-regression, determinism, scalar-fallback coverage): **pass**.

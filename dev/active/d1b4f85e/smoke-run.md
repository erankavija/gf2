# BCH workload bench smoke run (d1b4f85e)

This file records that the W1 and W2 bench targets build, run every
registered cell, and write a complete dispatch record. It is a smoke run with
one-second warm-up and measurement windows on a shared, unpinned host. It is
**not a measurement receipt**; `fd9d5416` takes the measurements with
[`run.sh`](run.sh) in a benchmark window.

- Revision: `9eb76a8d608dc31dea0eaf386f9d820b51e7643e` (bench sources as committed)
- Host: AMD Ryzen 9 5900X 12-Core (24 threads), Linux 7.2.6-arch2-1 x86_64, rustc 1.97.0, Cargo `bench` profile, no CPU pinning, no host lock
- Smoke window: 2026-10-01T09:09:06Z to 09:39:38Z
- Commands, from the repository root:

```text
env RAYON_NUM_THREADS=6 GF2_BCH_DISPATCH_RECORD=<scratch>/w1.jsonl ./scripts/cargo-budget.sh cargo bench -p gf2-coding --features parallel --bench bch_encode_w1 -- --warm-up-time 1 --measurement-time 1 --sample-size 10 --noplot
env GF2_BCH_DISPATCH_RECORD=<scratch>/w2.jsonl ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench bch_genmatrix -- --warm-up-time 1 --measurement-time 1 --sample-size 10 --noplot
env GF2_BCH_DISPATCH_RECORD=<scratch>/bo.jsonl ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench batch_operations -- bch_ --warm-up-time 1 --measurement-time 1 --sample-size 10 --noplot
GF2_BENCH=1 target/release/deps/bch_genmatrix-<hash> --bench --test 'bch_genmatrix_w2/materialize/.*/T2N$'
```

A group that sets its own sample count (ten on the large rows, a hundred
elsewhere) keeps it over `--sample-size`.

## Outcome

- All three targets exit 0. The W1 target registers 256 cells, the W2 target
  50 without `GF2_BENCH` (58 with it), and `batch_operations` 6 BCH cells;
  every cell has a dispatch-record line, and no ID repeats.
- Every W1 record reports `rayon_pool_width` 6. The output digests agree
  across every path of each row and batch; the bench asserts it before
  measuring, so a disagreement aborts the run.
- The detected encode kernel bundle is `avx2-pclmul`; each kernel-bundle
  family also runs its `scalar` arm.
- Under the conservative profile every `selected=` cell reports
  `poly-remainder-scalar`.
- The `GF2_BENCH=1` shortened `T2N` W2 cells (`materialize/fresh-alloc/T2N`,
  `materialize/warm-reuse/T2N`) pass in `--test` mode, about a minute per
  iteration; they have no smoke time below.

## Full-run wall-time estimate

From the smoke per-iteration times under the run's Criterion defaults (3 s
warm-up, 5 s measurement, flat sampling where an iteration outlasts the
target): about 57 minutes for the 312 cells below, about 20 minutes for the
two shortened `T2N` W2 cells, a few minutes for the `T2N-mother` W2 cells,
the `bch_parallel` decode groups and per-cell setup, so **about 90 minutes**
in total plus the pre-lock build. This is an estimate, not a measurement.

## Criterion IDs and smoke times (low / mid / high), not a measurement receipt

| ID | time |
|---|---|
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=1` | 230.41 ns / 230.60 ns / 230.80 ns |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=1` | 367.21 ns / 368.10 ns / 369.15 ns |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=1` | 54.896 ns / 55.032 ns / 55.180 ns |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=1` | 77.062 ns / 77.247 ns / 77.438 ns |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=1` | 64.227 ns / 64.396 ns / 64.593 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=1` | 82.135 ns / 82.268 ns / 82.403 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=1` | 69.111 ns / 69.204 ns / 69.300 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=1` | 1.3169 µs / 1.3209 µs / 1.3253 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=16` | 622.39 ns / 625.56 ns / 629.04 ns |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=16` | 768.54 ns / 771.33 ns / 774.81 ns |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=16` | 587.16 ns / 588.33 ns / 589.89 ns |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=16` | 951.72 ns / 953.57 ns / 955.51 ns |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=16` | 629.18 ns / 630.99 ns / 632.94 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=16` | 860.27 ns / 861.51 ns / 863.08 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=16` | 5.7946 µs / 5.8974 µs / 5.9987 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=16` | 8.2293 µs / 8.3826 µs / 8.5466 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=256` | 7.9848 µs / 7.9937 µs / 8.0039 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=256` | 8.6370 µs / 8.6618 µs / 8.6934 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=256` | 9.3438 µs / 9.3551 µs / 9.3665 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=256` | 15.584 µs / 15.642 µs / 15.731 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=256` | 10.030 µs / 10.050 µs / 10.071 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=256` | 14.959 µs / 14.970 µs / 14.983 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=256` | 10.241 µs / 10.413 µs / 10.596 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=256` | 27.094 µs / 28.200 µs / 29.382 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B1/B=4096` | 175.66 µs / 176.54 µs / 177.89 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B1/B=4096` | 182.74 µs / 183.05 µs / 183.50 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B1/B=4096` | 196.22 µs / 196.64 µs / 197.16 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B1/B=4096` | 295.33 µs / 295.60 µs / 295.91 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B1/B=4096` | 212.99 µs / 213.32 µs / 213.71 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B1/B=4096` | 298.52 µs / 300.15 µs / 301.98 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B1/B=4096` | 90.186 µs / 92.371 µs / 94.589 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B1/B=4096` | 244.49 µs / 249.96 µs / 255.15 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=1` | 829.94 ns / 831.56 ns / 833.57 ns |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=1` | 1.1896 µs / 1.1935 µs / 1.2008 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=1` | 239.13 ns / 239.42 ns / 239.70 ns |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=1` | 295.31 ns / 296.32 ns / 297.61 ns |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=1` | 251.50 ns / 252.38 ns / 253.51 ns |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=1` | 363.97 ns / 365.19 ns / 366.74 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=1` | 366.77 ns / 367.24 ns / 367.75 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=1` | 367.17 ns / 368.34 ns / 369.81 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=1` | 25.118 µs / 25.161 µs / 25.215 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=16` | 3.4029 µs / 3.4246 µs / 3.4617 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=16` | 3.7581 µs / 3.8026 µs / 3.8549 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=16` | 2.8311 µs / 2.8337 µs / 2.8363 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=16` | 3.6972 µs / 3.7037 µs / 3.7128 µs |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=16` | 2.9681 µs / 2.9935 µs / 3.0265 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=16` | 4.5082 µs / 4.5119 µs / 4.5159 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=16` | 4.6564 µs / 4.6629 µs / 4.6705 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=16` | 6.4757 µs / 6.6041 µs / 6.7310 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=16` | 44.955 µs / 45.559 µs / 46.125 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=256` | 95.244 µs / 96.355 µs / 97.796 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=256` | 98.661 µs / 99.278 µs / 100.11 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=256` | 87.462 µs / 87.569 µs / 87.670 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=256` | 117.67 µs / 119.24 µs / 120.58 µs |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=256` | 109.11 µs / 109.38 µs / 109.78 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=256` | 168.26 µs / 168.87 µs / 169.39 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=256` | 162.36 µs / 164.19 µs / 166.24 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=256` | 38.368 µs / 38.872 µs / 39.453 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=256` | 78.044 µs / 79.983 µs / 81.864 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B2/B=4096` | 2.0818 ms / 2.0837 ms / 2.0858 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B2/B=4096` | 2.1088 ms / 2.1112 ms / 2.1138 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B2/B=4096` | 1.9315 ms / 1.9331 ms / 1.9349 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B2/B=4096` | 2.1568 ms / 2.1596 ms / 2.1636 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B2/B=4096` | 1.9698 ms / 1.9769 ms / 1.9854 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B2/B=4096` | 2.9945 ms / 3.0023 ms / 3.0117 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B2/B=4096` | 3.0941 ms / 3.0986 ms / 3.1037 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B2/B=4096` | 643.41 µs / 663.47 µs / 689.10 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B2/B=4096` | 734.93 µs / 744.41 µs / 754.46 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=1` | 2.0371 µs / 2.0416 µs / 2.0472 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=1` | 2.3776 µs / 2.3820 µs / 2.3863 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=1` | 372.97 ns / 373.99 ns / 375.42 ns |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=1` | 581.41 ns / 582.31 ns / 583.24 ns |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=1` | 483.77 ns / 484.27 ns / 484.80 ns |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=1` | 761.83 ns / 763.10 ns / 764.49 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=1` | 773.95 ns / 774.58 ns / 775.28 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=1` | 762.89 ns / 766.71 ns / 771.70 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=1` | 25.172 µs / 25.224 µs / 25.288 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=16` | 7.1595 µs / 7.1731 µs / 7.1865 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=16` | 7.4222 µs / 7.4318 µs / 7.4420 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=16` | 5.3763 µs / 5.3823 µs / 5.3900 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=16` | 8.7475 µs / 8.7523 µs / 8.7578 µs |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=16` | 6.8264 µs / 6.8327 µs / 6.8400 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=16` | 11.299 µs / 11.315 µs / 11.334 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=16` | 11.410 µs / 11.421 µs / 11.434 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=16` | 7.6238 µs / 7.7882 µs / 7.9397 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=16` | 46.125 µs / 46.728 µs / 47.305 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=256` | 229.06 µs / 229.31 µs / 229.59 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=256` | 223.33 µs / 224.38 µs / 226.02 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=256` | 201.80 µs / 202.01 µs / 202.24 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=256` | 258.52 µs / 258.99 µs / 259.53 µs |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=256` | 241.32 µs / 241.41 µs / 241.51 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=256` | 427.93 µs / 428.29 µs / 428.66 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=256` | 431.59 µs / 431.95 µs / 432.33 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=256` | 87.334 µs / 87.642 µs / 88.039 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=256` | 128.15 µs / 130.11 µs / 132.26 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/B3/B=4096` | 3.9730 ms / 3.9756 ms / 3.9785 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/B3/B=4096` | 3.9838 ms / 3.9866 ms / 3.9895 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/B3/B=4096` | 3.7953 ms / 3.8046 ms / 3.8184 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=4096` | 4.6316 ms / 4.6369 ms / 4.6425 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/B3/B=4096` | 4.4020 ms / 4.4060 ms / 4.4108 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/B3/B=4096` | 7.4037 ms / 7.4213 ms / 7.4415 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/B3/B=4096` | 7.4621 ms / 7.4666 ms / 7.4715 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/B3/B=4096` | 1.3656 ms / 1.3810 ms / 1.3982 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/B3/B=4096` | 1.4899 ms / 1.5008 ms / 1.5136 ms |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=1` | 428.48 µs / 429.41 µs / 430.57 µs |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=1` | 563.50 µs / 568.98 µs / 577.73 µs |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=1` | 22.099 µs / 22.149 µs / 22.195 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=1` | 49.064 µs / 49.119 µs / 49.177 µs |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=1` | 34.313 µs / 34.371 µs / 34.450 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=1` | 132.34 µs / 132.66 µs / 133.01 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=1` | 132.96 µs / 134.00 µs / 135.47 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=1` | 132.39 µs / 132.89 µs / 133.46 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=1` | 167.78 µs / 168.25 µs / 168.86 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=16` | 1.2371 ms / 1.2398 ms / 1.2428 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=16` | 1.4020 ms / 1.4058 ms / 1.4095 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=16` | 932.97 µs / 940.39 µs / 949.62 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=16` | 1.3630 ms / 1.3672 ms / 1.3717 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=16` | 1.1325 ms / 1.1360 ms / 1.1399 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=16` | 2.5315 ms / 2.5431 ms / 2.5561 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=16` | 2.5188 ms / 2.5289 ms / 2.5425 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=16` | 544.65 µs / 554.87 µs / 565.38 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=16` | 552.86 µs / 565.37 µs / 581.91 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=256` | 16.259 ms / 16.283 ms / 16.308 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=256` | 16.929 ms / 17.006 ms / 17.095 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=256` | 14.967 ms / 15.029 ms / 15.111 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=256` | 21.998 ms / 22.116 ms / 22.269 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=256` | 18.210 ms / 18.270 ms / 18.329 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=256` | 40.328 ms / 40.478 ms / 40.650 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=256` | 40.248 ms / 40.322 ms / 40.405 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=256` | 7.2409 ms / 7.3730 ms / 7.5348 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=256` | 7.3878 ms / 7.5032 ms / 7.6525 ms |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=4096` | 261.66 ms / 262.04 ms / 262.42 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2S-mother/B=4096` | 272.04 ms / 272.17 ms / 272.30 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2S-mother/B=4096` | 240.52 ms / 241.38 ms / 242.23 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2S-mother/B=4096` | 351.04 ms / 352.41 ms / 354.03 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2S-mother/B=4096` | 292.67 ms / 293.50 ms / 294.53 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2S-mother/B=4096` | 651.78 ms / 653.76 ms / 656.25 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2S-mother/B=4096` | 654.81 ms / 669.16 ms / 685.97 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2S-mother/B=4096` | 117.86 ms / 118.85 ms / 119.91 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2S-mother/B=4096` | 120.57 ms / 122.15 ms / 124.50 ms |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=1` | 1.5942 ms / 1.5984 ms / 1.6038 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=1` | 2.3331 ms / 2.3365 ms / 2.3403 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=1` | 207.09 µs / 209.47 µs / 212.17 µs |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=1` | 323.25 µs / 324.18 µs / 325.40 µs |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=1` | 256.71 µs / 257.01 µs / 257.32 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=1` | 619.16 µs / 624.51 µs / 628.83 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=1` | 627.95 µs / 629.53 µs / 631.11 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=1` | 631.10 µs / 634.95 µs / 640.14 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=1` | 641.60 µs / 644.22 µs / 647.18 µs |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=16` | 5.1279 ms / 5.1384 ms / 5.1487 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=16` | 5.8729 ms / 5.8879 ms / 5.9016 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=16` | 3.9639 ms / 3.9911 ms / 4.0216 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=16` | 5.8167 ms / 5.9822 ms / 6.1508 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=16` | 5.9722 ms / 6.5959 ms / 7.3270 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=16` | 10.290 ms / 10.395 ms / 10.534 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=16` | 10.159 ms / 10.174 ms / 10.197 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=16` | 2.2663 ms / 2.3141 ms / 2.3586 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=16` | 2.1853 ms / 2.2469 ms / 2.3076 ms |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=256` | 66.268 ms / 66.355 ms / 66.448 ms |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=256` | 69.004 ms / 69.322 ms / 69.756 ms |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=256` | 60.258 ms / 60.411 ms / 60.576 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=256` | 90.568 ms / 91.784 ms / 93.391 ms |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=256` | 76.791 ms / 77.530 ms / 78.280 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=256` | 167.34 ms / 169.79 ms / 171.97 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=256` | 163.23 ms / 163.59 ms / 163.94 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=256` | 30.361 ms / 30.696 ms / 31.096 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=256` | 29.459 ms / 29.963 ms / 30.751 ms |
| `bch_encode_w1/family=bitslice-interleaved[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=4096` | 1.0592 s / 1.0614 s / 1.0635 s |
| `bch_encode_w1/family=bitslice-interleaved[scalar]/W1/warm-reuse/T2N-mother/B=4096` | 1.1120 s / 1.1141 s / 1.1162 s |
| `bch_encode_w1/family=clmul-fold[avx2-pclmul]/W1/warm-reuse/T2N-mother/B=4096` | 964.41 ms / 966.68 ms / 969.09 ms |
| `bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/T2N-mother/B=4096` | 1.4404 s / 1.4428 s / 1.4453 s |
| `bch_encode_w1/family=table-remainder/W1/warm-reuse/T2N-mother/B=4096` | 1.1998 s / 1.2271 s / 1.2574 s |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/T2N-mother/B=4096` | 2.8530 s / 3.1034 s / 3.3646 s |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/T2N-mother/B=4096` | 2.6765 s / 2.7284 s / 2.7856 s |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/T2N-mother/B=4096` | 472.43 ms / 481.36 ms / 492.10 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/T2N-mother/B=4096` | 486.56 ms / 502.71 ms / 524.95 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=1` | 155.98 µs / 159.43 µs / 162.97 µs |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=1` | 150.24 µs / 152.06 µs / 153.60 µs |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=16` | 2.9039 ms / 2.9445 ms / 2.9884 ms |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=16` | 2.8745 ms / 2.9047 ms / 2.9365 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=256` | 49.747 ms / 55.086 ms / 62.706 ms |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=256` | 175.14 ms / 222.44 ms / 274.14 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=4096` | 756.23 ms / 792.84 ms / 843.71 ms |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2S/B=4096` | 772.61 ms / 833.05 ms / 906.39 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=1` | 908.69 µs / 996.09 µs / 1.0821 ms |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=1` | 1.0559 ms / 1.0998 ms / 1.1434 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=16` | 18.849 ms / 19.239 ms / 19.641 ms |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=16` | 23.344 ms / 24.824 ms / 26.379 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=256` | 293.87 ms / 299.30 ms / 303.25 ms |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=256` | 199.41 ms / 200.08 ms / 200.84 ms |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2N/B=4096` | 3.3377 s / 3.4563 s / 3.6542 s |
| `bch_encode_w1/route=shortened-restriction/W1/warm-reuse/T2N/B=4096` | 3.2089 s / 3.2436 s / 3.2786 s |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=1` | 97.172 ns / 97.324 ns / 97.485 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=1` | 628.15 ns / 809.94 ns / 993.42 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=1` | 184.13 ns / 189.38 ns / 199.46 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=1` | 308.01 ns / 312.28 ns / 317.59 ns |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=16` | 1.3225 µs / 1.3257 µs / 1.3301 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=16` | 1.3820 µs / 1.3862 µs / 1.3913 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=16` | 6.8542 µs / 7.0243 µs / 7.2380 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=16` | 8.9908 µs / 9.5635 µs / 10.217 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=256` | 20.053 µs / 20.105 µs / 20.173 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=256` | 24.703 µs / 24.803 µs / 24.965 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=256` | 15.281 µs / 15.477 µs / 15.701 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=256` | 34.073 µs / 35.145 µs / 36.248 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N1/B=4096` | 311.77 µs / 312.14 µs / 312.72 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N1/B=4096` | 390.03 µs / 390.45 µs / 390.91 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N1/B=4096` | 158.32 µs / 160.63 µs / 163.04 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N1/B=4096` | 410.57 µs / 423.27 µs / 434.58 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=1` | 351.46 ns / 353.25 ns / 356.59 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=1` | 362.28 ns / 362.77 ns / 363.27 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=1` | 359.73 ns / 360.93 ns / 362.67 ns |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=1` | 645.83 ns / 646.54 ns / 647.23 ns |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=16` | 5.2608 µs / 5.2751 µs / 5.2911 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=16` | 5.3830 µs / 5.3929 µs / 5.4062 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=16` | 6.5894 µs / 6.7326 µs / 6.8876 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=16` | 8.3228 µs / 8.4991 µs / 8.6719 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=256` | 92.479 µs / 93.865 µs / 95.051 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=256` | 93.588 µs / 94.769 µs / 96.343 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=256` | 37.221 µs / 38.050 µs / 38.796 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=256` | 54.549 µs / 55.843 µs / 57.126 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N2/B=4096` | 1.3830 ms / 1.3897 ms / 1.3966 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N2/B=4096` | 1.5602 ms / 1.6478 ms / 1.7500 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N2/B=4096` | 602.30 µs / 608.94 µs / 616.18 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N2/B=4096` | 733.10 µs / 743.17 µs / 754.31 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=1` | 1.4572 µs / 1.4586 µs / 1.4600 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=1` | 1.7204 µs / 1.7284 µs / 1.7362 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=1` | 1.4877 µs / 1.4891 µs / 1.4907 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=1` | 2.9322 µs / 2.9503 µs / 2.9710 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=16` | 22.336 µs / 22.390 µs / 22.434 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=16` | 34.026 µs / 34.106 µs / 34.185 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=16` | 53.418 µs / 53.915 µs / 54.501 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=16` | 67.274 µs / 68.375 µs / 69.489 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=256` | 366.69 µs / 367.07 µs / 367.58 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=256` | 575.83 µs / 580.40 µs / 585.11 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=256` | 717.12 µs / 719.50 µs / 722.62 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=256` | 955.69 µs / 973.50 µs / 991.71 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N3/B=4096` | 7.5339 ms / 7.5912 ms / 7.6590 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N3/B=4096` | 8.9526 ms / 9.2758 ms / 9.6216 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N3/B=4096` | 12.636 ms / 12.777 ms / 12.929 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N3/B=4096` | 14.912 ms / 15.082 ms / 15.261 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=1` | 86.235 µs / 86.699 µs / 87.157 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=1` | 86.634 µs / 87.054 µs / 87.469 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=1` | 89.093 µs / 89.145 µs / 89.200 µs |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=1` | 128.25 µs / 136.99 µs / 146.03 µs |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=16` | 2.0730 ms / 2.2407 ms / 2.4549 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=16` | 1.4628 ms / 1.4681 ms / 1.4739 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=16` | 8.4996 ms / 8.6014 ms / 8.7057 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=16` | 4.7907 ms / 5.3285 ms / 5.9041 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=256` | 83.486 ms / 96.842 ms / 111.00 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=256` | 52.529 ms / 57.770 ms / 63.550 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=256` | 113.02 ms / 118.70 ms / 124.68 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=256` | 106.22 ms / 112.38 ms / 119.01 ms |
| `bch_encode_w1/family=poly-remainder-scalar/W1/warm-reuse/N4/B=4096` | 449.48 ms / 491.27 ms / 538.15 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W1/fresh-alloc/N4/B=4096` | 367.13 ms / 370.40 ms / 373.88 ms |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/warm-reuse/N4/B=4096` | 2.2594 s / 2.3198 s / 2.3851 s |
| `bch_encode_w1/selected=poly-remainder-scalar/W6/fresh-alloc/N4/B=4096` | 2.2236 s / 2.2609 s / 2.3019 s |
| `bch_genmatrix_w2/materialize/fresh-alloc/B1` | 42.904 ns / 42.948 ns / 42.986 ns |
| `bch_genmatrix_w2/reference/fresh-alloc/B1` | 440.18 ns / 441.70 ns / 443.62 ns |
| `bch_genmatrix_w2/materialize/warm-reuse/B1` | 35.898 ns / 35.927 ns / 35.961 ns |
| `bch_genmatrix_w2/reference/warm-reuse/B1` | 439.81 ns / 440.62 ns / 441.56 ns |
| `bch_genmatrix_w2/materialize/fresh-alloc/B2` | 465.96 ns / 468.08 ns / 471.93 ns |
| `bch_genmatrix_w2/reference/fresh-alloc/B2` | 43.434 µs / 43.958 µs / 44.759 µs |
| `bch_genmatrix_w2/materialize/warm-reuse/B2` | 465.23 ns / 467.36 ns / 471.05 ns |
| `bch_genmatrix_w2/reference/warm-reuse/B2` | 40.932 µs / 41.329 µs / 41.844 µs |
| `bch_genmatrix_w2/materialize/fresh-alloc/B3` | 1.8198 µs / 1.9478 µs / 2.0784 µs |
| `bch_genmatrix_w2/reference/fresh-alloc/B3` | 392.34 µs / 415.61 µs / 439.69 µs |
| `bch_genmatrix_w2/materialize/warm-reuse/B3` | 2.0405 µs / 2.0993 µs / 2.1536 µs |
| `bch_genmatrix_w2/reference/warm-reuse/B3` | 546.04 µs / 552.47 µs / 558.56 µs |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2S-mother` | 4.7845 ms / 5.0265 ms / 5.1808 ms |
| `bch_genmatrix_w2/reference/fresh-alloc/T2S-mother` | 2.0202 s / 2.2422 s / 2.6356 s |
| `bch_genmatrix_w2/materialize/warm-reuse/T2S-mother` | 854.62 µs / 1.0022 ms / 1.1289 ms |
| `bch_genmatrix_w2/reference/warm-reuse/T2S-mother` | 2.1567 s / 2.4844 s / 2.8357 s |
| `bch_genmatrix_w2/materialize/fresh-alloc/N1` | 77.287 ns / 77.528 ns / 77.823 ns |
| `bch_genmatrix_w2/reference/fresh-alloc/N1` | 611.94 ns / 715.24 ns / 845.75 ns |
| `bch_genmatrix_w2/materialize/warm-reuse/N1` | 78.971 ns / 81.923 ns / 85.929 ns |
| `bch_genmatrix_w2/reference/warm-reuse/N1` | 343.88 ns / 345.39 ns / 347.32 ns |
| `bch_genmatrix_w2/materialize/fresh-alloc/N2` | 551.03 ns / 553.03 ns / 555.53 ns |
| `bch_genmatrix_w2/reference/fresh-alloc/N2` | 6.6627 µs / 6.7029 µs / 6.7664 µs |
| `bch_genmatrix_w2/materialize/warm-reuse/N2` | 458.74 ns / 459.93 ns / 461.31 ns |
| `bch_genmatrix_w2/reference/warm-reuse/N2` | 6.6354 µs / 6.6699 µs / 6.7244 µs |
| `bch_genmatrix_w2/materialize/fresh-alloc/N3` | 3.6851 µs / 4.0209 µs / 4.3554 µs |
| `bch_genmatrix_w2/reference/fresh-alloc/N3` | 14.129 µs / 14.144 µs / 14.158 µs |
| `bch_genmatrix_w2/materialize/warm-reuse/N3` | 2.3492 µs / 2.3546 µs / 2.3611 µs |
| `bch_genmatrix_w2/reference/warm-reuse/N3` | 15.658 µs / 15.731 µs / 15.805 µs |
| `bch_genmatrix_w2/materialize/fresh-alloc/N4` | 587.88 µs / 588.42 µs / 588.98 µs |
| `bch_genmatrix_w2/reference/fresh-alloc/N4` | 24.964 ms / 25.114 ms / 25.273 ms |
| `bch_genmatrix_w2/materialize/warm-reuse/N4` | 343.35 µs / 345.42 µs / 347.63 µs |
| `bch_genmatrix_w2/reference/warm-reuse/N4` | 24.369 ms / 24.467 ms / 24.581 ms |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2S` | 1.9081 s / 1.9153 s / 1.9237 s |
| `bch_genmatrix_w2/materialize/warm-reuse/T2S` | 2.0655 s / 3.2255 s / 5.1730 s |
| `bch_paritycheck/materialize/warm-reuse/B1` | 105.99 ns / 106.38 ns / 106.95 ns |
| `bch_paritycheck/reference/warm-reuse/B1` | 536.48 ns / 540.49 ns / 545.65 ns |
| `bch_paritycheck/materialize/warm-reuse/B2` | 23.402 µs / 26.359 µs / 29.581 µs |
| `bch_paritycheck/reference/warm-reuse/B2` | 94.666 µs / 110.75 µs / 129.33 µs |
| `bch_paritycheck/materialize/warm-reuse/B3` | 10.836 µs / 12.006 µs / 13.391 µs |
| `bch_paritycheck/reference/warm-reuse/B3` | 174.07 µs / 175.34 µs / 176.76 µs |
| `bch_paritycheck/materialize/warm-reuse/T2S-mother` | 12.929 ms / 13.403 ms / 14.243 ms |
| `bch_paritycheck/reference/warm-reuse/T2S-mother` | 1.6365 s / 1.8582 s / 2.0822 s |
| `bch_paritycheck/materialize/warm-reuse/N1` | 80.544 ns / 84.578 ns / 89.628 ns |
| `bch_paritycheck/reference/warm-reuse/N1` | 410.61 ns / 413.60 ns / 416.87 ns |
| `bch_paritycheck/materialize/warm-reuse/N2` | 368.17 ns / 378.18 ns / 396.33 ns |
| `bch_paritycheck/reference/warm-reuse/N2` | 7.5751 µs / 7.8505 µs / 8.1897 µs |
| `bch_paritycheck/materialize/warm-reuse/N3` | 2.0175 µs / 2.0406 µs / 2.0722 µs |
| `bch_paritycheck/reference/warm-reuse/N3` | 15.459 µs / 15.481 µs / 15.505 µs |
| `bch_paritycheck/materialize/warm-reuse/N4` | 164.90 µs / 165.50 µs / 166.23 µs |
| `bch_paritycheck/reference/warm-reuse/N4` | 24.895 ms / 25.053 ms / 25.242 ms |
| `bch_encode_pns_16383_16215/1` | 68.546 µs / 68.817 µs / 69.316 µs |
| `bch_encode_pns_16383_16215/10` | 658.22 µs / 664.93 µs / 679.11 µs |
| `bch_encode_pns_16383_16215/50` | 3.3862 ms / 3.3892 ms / 3.3919 ms |
| `bch_encode_pns_16383_16215/100` | 6.7710 ms / 6.7751 ms / 6.7776 ms |
| `bch_sequential_vs_batch/sequential_loop` | 9.9886 µs / 10.014 µs / 10.044 µs |
| `bch_sequential_vs_batch/batch_operation` | 6.6972 µs / 6.7367 µs / 6.8165 µs |

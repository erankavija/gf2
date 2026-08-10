# Provenance-fixed four-matrix $\mathbb{F}_3$ AVX2 permanent receipt

This is the authoritative receipt for the one-word, four-matrix F_3 permanent
path after the full-worktree provenance correction. The canonical raw data is
[`batched-f3-avx2-provenance-fixed.csv`](batched-f3-avx2-provenance-fixed.csv),
whose SHA-256 is
`c05b0eb8f5e31cd299ad5ea13526e8e1c5a79bd7ca6cc1839f64cd09925be35c`.

## Result

The batched AVX2 path leads scalar single-word at every measured one-word size:
3.572785x at $n=8$, then 5.293715x, 6.045207x, 6.162474x, 6.173246x, and
6.167938x through $n=28$. It also leads direct single-matrix AVX2 at every
size, by 10.212946x--20.707052x. There is no batching non-lead.

The four-lane expectation is not contradicted: batching pays at all six sizes.
It is not an exact fourfold law: batched/scalar is below 4x at $n=8$ and above
4x from $n=12$ onward. The direct single-matrix AVX2 kernel is slower than
scalar by 2.858539x--3.354322x in pooled-rate comparison. The earlier
provenance-incomplete cohorts remain non-authoritative and are not used here.

## Reproducible protocol and provenance

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-algebra/benches/batched_f3_permanent.rs`; schema `batched-f3-avx2-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Clean source revision | `88474a74ceee817040327db164c21f9fdd5ccf84`; every row records `source_dirty=false` |
| Bench binary SHA-256 | `6ba61fd61e04041545836e7c000e451208616602d31fa8a8430b33cb9ce2ceb3` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Seed root and fixtures | `0xddd0c6ee00000000`; 32 deterministic fixture groups; four matrices per call |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor; AVX2 |
| OS/kernel and governor | Linux 7.1.6-arch1-1, `#1 SMP PREEMPT_DYNAMIC Tue, 04 Aug 2026 11:19:27 +0000 x86_64 GNU/Linux`; `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`; CPUs 6--11 |
| Niceness | The wrapper's best-effort `nice -n -5` was denied; the lock and affinity remained active |
| Timed work | Five fresh executions, five repetitions each, `--target-ms 250` |
| Measured duration | 2026-08-10 02:51:00--03:04:38 UTC (about 13 min 39 s) |

Each execution/repetition uses
`fixture_start = ((execution - 1) * 5 + (repetition - 1)) & 31`. The raw file
contains exactly 450 unique coordinates:
`execution=1..5` x `repetition=1..5` x
`n in {8, 12, 16, 20, 24, 28}` x the three named backends. Before every timed
coordinate, the harness checks equivalence of batched AVX2, scalar single-word,
and direct single-matrix AVX2 on the selected four-matrix fixture. The output
was first written to a unique absent `/tmp` file, then copied byte-for-byte to
the canonical raw path after validation.

The command was:

```sh
test ! -e /tmp/gf2-ddd0c6ee-batched-f3-avx2-provenance-fixed-88474a74.csv
./dev/scripts/ccx1-bench-flock.sh bash -lc 'for e in 1 2 3 4 5; do cargo +1.95.0 bench -p gf2-algebra --bench batched_f3_permanent --features simd,test-support -- --execution "$e" --repetitions 5 --target-ms 250 --output /tmp/gf2-ddd0c6ee-batched-f3-avx2-provenance-fixed-88474a74.csv --append; done'
```

The previous raw cohort is retained and explicitly not interpreted:
[provenance-incomplete record](batched-f3-avx2.md). The earlier source-split
attempt is also preserved separately in that record.

## Pooled per-matrix rates

Each rate is `sum(matrices) / sum(elapsed_ns) * 1e9`; every speedup is the
ratio of those pooled rates, never a mean of reciprocal per-row timings. `B/S`
is batched/scalar, `B/D` batched/direct single-matrix AVX2, and `S/D`
scalar/direct AVX2.

| n | Batched AVX2 matrices/s | Scalar matrices/s | Direct AVX2 matrices/s | B/S | B/D | S/D |
|---:|---:|---:|---:|---:|---:|---:|
| 8 | 2,763,509.697 | 773,488.843 | 270,588.884 | 3.572785x | 10.212946x | 2.858539x |
| 12 | 298,075.609 | 56,307.458 | 17,748.839 | 5.293715x | 16.794090x | 3.172459x |
| 16 | 22,283.147 | 3,686.085 | 1,125.980 | 6.045207x | 19.790003x | 3.273668x |
| 20 | 1,428.796 | 231.854 | 70.214 | 6.162474x | 20.349073x | 3.302095x |
| 24 | 89.536 | 14.504 | 4.324 | 6.173246x | 20.707052x | 3.354322x |
| 28 | 5.585 | 0.906 | 0.270 | 6.167938x | 20.684921x | 3.353620x |

## Dispersion

Within-execution dispersion is the minimum--maximum sample coefficient of
variation (standard deviation divided by mean) across the five repetitions in
each execution. Across-execution dispersion is the sample coefficient of
variation of the five execution-level pooled rates. They are deliberately
reported separately.

| n | Batched within / across | Scalar within / across | Direct AVX2 within / across |
|---:|---:|---:|---:|
| 8 | 0.054--0.255% / 0.215% | 0.080--0.224% / 0.216% | 0.096--0.244% / 12.818% |
| 12 | 0.134--0.463% / 0.484% | 0.021--0.100% / 0.084% | 0.032--0.099% / 13.582% |
| 16 | 0.022--0.310% / 0.127% | 0.021--0.087% / 0.050% | 0.032--0.496% / 13.831% |
| 20 | 0.048--0.279% / 0.111% | 0.038--0.194% / 0.073% | 0.087--0.782% / 13.862% |
| 24 | 0.062--0.194% / 0.157% | 0.049--0.106% / 0.045% | 0.055--0.286% / 13.891% |
| 28 | 0.024--0.156% / 0.226% | 0.024--0.224% / 0.062% | 0.044--0.121% / 13.914% |

Direct single-matrix AVX2 has materially larger process-to-process dispersion,
but no execution-level or pooled ordering reverses: batched leads scalar and
direct at all six sizes.

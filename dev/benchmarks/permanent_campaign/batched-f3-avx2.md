# Four-matrix $\mathbb{F}_3$ AVX2 permanent receipt

This is the authoritative receipt for the one-word, four-matrix F_3
permanent path.  The canonical raw data is
[`batched-f3-avx2.csv`](batched-f3-avx2.csv), whose SHA-256 is
`f6c5eb673e982f002ab71002f4310fc7db7f31b320850a70bd2526e2248742ee`.

## Result

The batched AVX2 path leads the scalar single-word path at **every** measured
one-word size: 3.608645x at $n=8$, increasing to 6.202144x at $n=28$.  It also
leads the direct single-matrix AVX2 kernel at every size, by 9.699538x to
19.119803x.  Thus no measured size is a batching non-lead.

The four-lane expectation is not falsified: batching pays at all six sizes.
It is not an exact fourfold law: the batched/scalar gain is below 4x at $n=8$
and above 4x from $n=12$ onward.  The direct single-matrix AVX2 kernel remains
slower than scalar by 2.687862x to 3.088253x in pooled rate comparison.  This
preserves the prior dispatcher diagnosis rather than replacing it with an
unqualified SIMD claim.

## Reproducible protocol and provenance

| Item | Value |
|---|---|
| Harness | `crates/gf2-algebra/benches/batched_f3_permanent.rs`, schema `batched-f3-avx2-v1` |
| Clean source revision | `5b15d2723fcede3ccc082e508d1223c2f54087ce`; every row records `source_dirty=false` |
| Bench binary SHA-256 | `bd0294ff0a68a94f14715fa9216a48600aaeb535f5bf442ca628a832986be266` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Seed root | `0xddd0c6ee00000000` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor; AVX2 |
| OS/kernel | Linux 7.1.6-arch1-1, `#1 SMP PREEMPT_DYNAMIC Tue, 04 Aug 2026 11:19:27 +0000 x86_64 GNU/Linux` |
| CPU governor | `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`; CPUs 6--11 |
| Niceness | The wrapper's best-effort `nice -n -5` was denied; locking and affinity remained active |
| Timed work | 5 fresh benchmark executions, each with 5 repetitions and `--target-ms 250` |
| Measured duration | 2026-08-10 02:11:22--02:24:31 UTC (about 13 min 10 s) |

Before timing every coordinate, the harness checks that batched AVX2, scalar
single-word, and direct single-matrix AVX2 return equivalent permanent values
on the selected four-matrix fixture.  It uses 32 deterministic fixture groups;
for execution $e$ and repetition $r$, `fixture_start = ((e - 1) * 5 + (r - 1))
& 31`.  The raw file has exactly the Cartesian product
`execution=1..5` x `repetition=1..5` x
`n in {8, 12, 16, 20, 24, 28}` x the three backends: 450 unique rows, with four
matrices timed per call.  The output was first written to a unique absent
`/tmp` path and copied byte-for-byte to the canonical path above after the
cohort completed.

The command was:

```sh
test ! -e /tmp/gf2-ddd0c6ee-batched-f3-avx2-5b15d272.csv
./dev/scripts/ccx1-bench-flock.sh bash -lc 'for e in 1 2 3 4 5; do cargo +1.95.0 bench -p gf2-algebra --bench batched_f3_permanent --features simd,test-support -- --execution "$e" --repetitions 5 --target-ms 250 --output /tmp/gf2-ddd0c6ee-batched-f3-avx2-5b15d272.csv --append; done'
```

An earlier source-split attempt is retained, but is expressly non-authoritative
and not interpreted: [interrupted-attempt record](batched-f3-avx2-interrupted-2026-08-10.md).

## Pooled rates

Each rate is `sum(matrices) / sum(elapsed_ns) * 1e9`; each speedup is the
corresponding ratio of those pooled rates.  Consequently the table does not
average per-row reciprocals.  `B/S` is batched over scalar, `B/D` batched over
direct single-matrix AVX2, and `S/D` scalar over direct AVX2.

| n | Batched AVX2 matrices/s | Scalar matrices/s | Direct AVX2 matrices/s | B/S | B/D | S/D |
|---:|---:|---:|---:|---:|---:|---:|
| 8 | 2,742,394.654 | 759,951.393 | 282,734.569 | 3.608645x | 9.699538x | 2.687862x |
| 12 | 295,154.039 | 55,569.514 | 18,651.671 | 5.311438x | 15.824536x | 2.979332x |
| 16 | 22,073.555 | 3,640.314 | 1,186.256 | 6.063640x | 18.607755x | 3.068743x |
| 20 | 1,416.146 | 229.129 | 74.194 | 6.180552x | 19.087112x | 3.088253x |
| 24 | 88.725 | 14.321 | 4.640 | 6.195578x | 19.119803x | 3.086040x |
| 28 | 5.540 | 0.893 | 0.290 | 6.202144x | 19.113931x | 3.081827x |

## Dispersion

Within-execution dispersion is the minimum--maximum coefficient of variation
(sample standard deviation divided by mean) over the five repetitions in each
of the five executions.  Across-execution dispersion is the coefficient of
variation of the five execution-level pooled rates.  These are reported
separately to avoid conflating repetition jitter with process-to-process
variation.

| n | Batched within / across | Scalar within / across | Direct AVX2 within / across |
|---:|---:|---:|---:|
| 8 | 0.060--0.208% / 0.099% | 0.151--0.242% / 1.534% | 0.114--0.556% / 1.133% |
| 12 | 0.029--0.284% / 0.193% | 0.069--0.196% / 0.090% | 0.072--0.749% / 0.893% |
| 16 | 0.083--0.124% / 0.152% | 0.092--0.183% / 0.107% | 0.051--0.489% / 1.245% |
| 20 | 0.105--0.422% / 0.233% | 0.126--0.235% / 0.089% | 0.067--0.847% / 1.037% |
| 24 | 0.101--0.449% / 0.076% | 0.120--0.274% / 0.061% | 0.038--0.262% / 1.043% |
| 28 | 0.050--0.148% / 0.064% | 0.029--0.084% / 0.047% | 0.016--0.105% / 0.931% |

The direct AVX2 backend has the larger across-execution dispersion, but no
dispersion interval reverses any pooled ordering in this receipt.

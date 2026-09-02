# W2 materialization receipt (`bd0edfa2`)

| Field | Value |
|---|---|
| JIT issue | `bd0edfa2` (Optimize the reference generator-matrix materialization) |
| Requirement | `@/issue/bd0edfa2/requirement/REQ-02` |
| Workload | `dev/active/4e732b56/workload-selection.md` § 1 W2, § 4 dimensions, § 5 cache states |
| Rows | § 2 B1, B2, B3 at the lengths it fixes; the DVB-T2 rows at the mother lengths § 9 fixes for this consumer |
| Measured revision | `5847c2e0b4b83e90afde080723648dd032a8c808` |
| Bench target | `crates/gf2-coding/benches/bch_genmatrix.rs` |
| Raw output | [`2026-09-02-bd0edfa2-criterion-raw.txt`](2026-09-02-bd0edfa2-criterion-raw.txt) |

## Host and protocol

| Field | Value |
|---|---|
| CPU | AMD Ryzen 9 5900X, 12 cores / 24 threads, 64 MiB L3 (2 instances) |
| Core pinning | `taskset -c 6-11` (CCX1), applied by `dev/scripts/ccx1-bench-flock.sh` |
| Serialization | `flock -x /tmp/gf2-ccx1.lock` held for each child |
| Priority | `nice -n -5` requested by the wrapper and denied to the non-root child; the child ran at the inherited default |
| Frequency governor | `powersave` (recorded, not changed) |
| OS | Arch Linux, Linux 7.1.11-arch1-1 x86_64 |
| Toolchain | rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0 |
| Profile | `--release` (Cargo `bench` profile: thin LTO, one codegen unit) |
| Worker count | 1; every cell is single-threaded |
| Window | 2026-09-02 11:27:56Z to 11:33:02Z, load average 1.16 at open and 1.10 at close, no other cargo, rustc, or test process on the host |
| Statistics | Criterion, 100 samples for B1–B3 and 10 for the two mother rows, 3 s warm-up and 5 s measurement per cell |
| Spread | Criterion's reported confidence interval as a percentage of the median |

`GF2_BENCH=1` selects the `T2N-mother` row, which materializes a 512 MiB
generator; the other rows run without it.

These cells follow the survey's measurement protocol
(`dev/active/4e732b56/findings.md` § 4) and differ from it in one respect: the
survey compares gf2 against external baselines, and this compares two gf2
paths against each other, so no external harness, fixture seed, or output
digest enters here.

## Cells

`materialization` is the canonical path, `GeneratorMatrixAccess` and
`ParityCheckMatrixAccess` on `crates/gf2-coding/src/bch/matrix.rs`.
`reference` is the basis-vector writer `gf2_coding::test_support` exposes,
which encodes one message basis vector per row. `fresh-alloc` is the
allocating accessor and `warm-reuse` the caller-buffer one, per § 5.
Throughput is matrix bits per second: $kn/T$ for the generator and $(n-k)n/T$
for the parity check.

| Group | Row | Cache state | Materialization median | Reference median | Ratio | Materialization | Reference | Spread (mat / ref) |
|---|---|---|---|---|---|---|---|---|
| generator | B1 | fresh-alloc | 43.74 ns | 456.4 ns | 10× | 1.714 Gbit/s | 164.3 Mbit/s | 0.23% / 0.13% |
| generator | B1 | warm-reuse | 36.76 ns | 452.2 ns | 12× | 2.040 Gbit/s | 165.9 Mbit/s | 0.18% / 0.13% |
| generator | B2 | fresh-alloc | 484.4 ns | 41.87 µs | 86× | 16.78 Gbit/s | 194.1 Mbit/s | 0.14% / 0.16% |
| generator | B2 | warm-reuse | 476.0 ns | 42.11 µs | 88× | 17.08 Gbit/s | 193.0 Mbit/s | 0.12% / 0.23% |
| generator | B3 | fresh-alloc | 1.522 µs | 296.5 µs | 195× | 37.36 Gbit/s | 191.8 Mbit/s | 0.11% / 0.11% |
| generator | B3 | warm-reuse | 1.467 µs | 294.4 µs | 201× | 38.77 Gbit/s | 193.2 Mbit/s | 0.14% / 0.08% |
| generator | T2S-mother | fresh-alloc | 2.419 ms | 2.073 s | 857× | 109.8 Gbit/s | 128.1 Mbit/s | 0.17% / 0.07% |
| generator | T2S-mother | warm-reuse | 684.6 µs | 2.045 s | 2987× | 388.0 Gbit/s | 129.9 Mbit/s | 0.89% / 0.06% |
| generator | T2N-mother | fresh-alloc | 36.45 ms | 33.67 s (estimate) | 924× (estimate) | 117.5 Gbit/s | 127.2 Mbit/s (estimate) | 0.23% / — |
| generator | T2N-mother | warm-reuse | 27.61 ms | 33.21 s (estimate) | 1203× (estimate) | 155.1 Gbit/s | 128.9 Mbit/s (estimate) | 0.34% / — |
| parity check | B1 | warm-reuse | 94.79 ns | 495.4 ns | 5× | 1.582 Gbit/s | 302.8 Mbit/s | 0.14% / 0.13% |
| parity check | B2 | warm-reuse | 5.616 µs | 34.34 µs | 6× | 1.425 Gbit/s | 233.0 Mbit/s | 0.18% / 0.16% |
| parity check | B3 | warm-reuse | 10.03 µs | 171.9 µs | 17× | 814.0 Mbit/s | 47.48 Mbit/s | 0.16% / 0.40% |
| parity check | T2S-mother | warm-reuse | 12.20 ms | 1.486 s | 122× | 225.6 Mbit/s | 1.853 Mbit/s | 0.31% / 0.49% |
| parity check | T2N-mother | warm-reuse | 78.90 ms | 24.13 s (estimate) | 306× (estimate) | 159.5 Mbit/s | 521.5 kbit/s (estimate) | 0.46% / — |

The materialization is faster than the reference in every cell. The smallest
ratio is 5× on the B1 parity check, where a 10 × 15 output leaves the
per-call cost dominant, and the largest measured ratio is 2987× on the T2S
mother generator under `warm-reuse`.

## The three projected cells

Each cell carries a 90 s wall budget, matching the survey's encoder-harness
budget. The three `T2N-mother` reference cells exceed one Criterion sample
run within it, so they are **projections from a measured per-unit cost,
labeled as estimates, not measurements**.

The reference spends $O(k^2 \lceil r/64 \rceil)$ word operations. Both mother
rows have $\lceil r/64 \rceil = 3$ ($r = 168$ and $192$), so the T2N unit
count is $(65343/16215)^2 = 16.2392$ times the T2S one, and each projection is
the measured T2S median scaled by that factor:

| Cell | Measured T2S reference | Scale | Projected T2N reference |
|---|---|---|---|
| generator, fresh-alloc | 2.0733 s | 16.2392 | 33.67 s |
| generator, warm-reuse | 2.0450 s | 16.2392 | 33.21 s |
| parity check, warm-reuse | 1.4857 s | 16.2392 | 24.13 s |

The projection assumes the reference's per-unit cost is flat between the two
shapes. The measured per-unit costs of the smaller rows are not flat with the
mother rows — B3's generator reference costs 5.92 ns per unit against T2S's
2.59 ns, because the per-encode overhead and the $O(kn)$ cell writes dominate
at $k = 223$ — so the projection is anchored on T2S alone, the nearest shape,
and every T2N reference figure above is an estimate.

## Reproduction

```
cargo bench -p gf2-coding --bench bch_genmatrix --no-run
./dev/scripts/ccx1-bench-flock.sh target/release/deps/bch_genmatrix-<hash> --bench --noplot
./dev/scripts/ccx1-bench-flock.sh env GF2_BENCH=1 \
    target/release/deps/bch_genmatrix-<hash> --bench --noplot 'materialize.*T2N-mother'
```

The absolute figures hold for this host under the `powersave` governor. The
ratios between the two paths are what REQ-02 rests on.

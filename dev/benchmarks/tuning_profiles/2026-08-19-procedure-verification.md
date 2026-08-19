# Procedure-verification receipt for the pinned selector non-regression set

This receipt records that
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md)
executes end to end on a prepared host and is reproducible from its committed
description. It is the execution evidence for issue `e8fe47f5` REQ-02.

**It is not the pre-cutover baseline.** The baseline is issue `278acf3a`, which
runs the same procedure from a clean revision; this run's rows record
`source_dirty=true`, because the procedure and its harness were still in the
working tree when it ran. Nothing here may be cited as a baseline for a cutover
comparison.

## Result

The procedure ran twice, producing two independent five-execution cohorts of
identical pre-cutover code, and the comparison rule of the plan's §5 was applied
to them. Every one of the thirty-four pinned cells passed the predeclared 5 %
per-cell tolerance, the geometric mean of the per-cell ratios was 1.000810
against the predeclared 2 % set tolerance, and the comparison reported
`RESULT: PASS`.

Both cohorts recorded the arm the plan's §2 predicts for every cell, so no cell
silently measured the wrong side of a threshold. Every cell's equivalence probe
passed, so both cohorts computed correct results throughout.

Because both cohorts measure the same code, their true per-cell ratio is 1 and
the observed spread is the protocol's own noise. The widest deviation is 1.91 %
at `bit_backend/xor_inplace/words=64`; the next widest is 1.15 % at
`polynomial/mul/len=64`. Every other cell lands inside 1 %.

## Reproducible protocol and provenance

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | [`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md), executed unmodified |
| Source revision | `1d8a289aeac59fa2454523816887797e96293eca`; every row records `source_dirty=true` (see above) |
| Bench binary SHA-256 | `890cddb092bf5ea2c3ec1cb2bd41cdcb93338a5d31e15668f92055b2951202d0` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0 bench` |
| Features | `simd` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`, uncontended at start; CPUs 6--11 |
| Niceness | The wrapper's best-effort `nice -n -5` was denied; the lock and affinity remained active |
| Timed work | Two cohorts, each five fresh executions of five repetitions, `--target-ms 250` |
| Measured duration | 2026-08-19 06:13:11--06:20:40 UTC (about 7 min 29 s for both cohorts) |
| Raw cohort A | [`2026-08-19-procedure-verification-cohort-a.csv`](2026-08-19-procedure-verification-cohort-a.csv), SHA-256 `44ff73c9e7b3af0aa4e6432514c43e3c20e9a1eda407b5712100811eabd6e627` |
| Raw cohort B | [`2026-08-19-procedure-verification-cohort-b.csv`](2026-08-19-procedure-verification-cohort-b.csv), SHA-256 `076cc83d87b51f7b22dbd70120a54abfb9238cba63c9654d260bff1a76f3724f` |

Each raw file holds 850 rows: 34 cells x 5 executions x 5 repetitions. Both were
written to a unique absent `/tmp` path and copied byte-for-byte after the run.

## Step 0 — harness against the plan

`--self-check` printed:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

The tolerances match the plan's §4, the cell count matches §2, and
`simd_min_words=8` is observed by probing `select_backend_for_size` rather than
read from a literal. `--list-cells` reproduced the plan's §2 table, all
thirty-four cells with the arms it predicts.

## Step 1 — commands

```sh
for COHORT in /tmp/gf2-e8fe47f5-procedure-verification-a-1d8a289a.csv \
              /tmp/gf2-e8fe47f5-procedure-verification-b-1d8a289a.csv; do
  GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash -lc '
    for e in 1 2 3 4 5; do
      cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
        --execution "$e" --repetitions 5 --target-ms 250 --output "'"$COHORT"'" --append
    done'
done
```

## Cohort comparison

Pooled nanoseconds per call, summing `elapsed_ns` and `calls` over all
twenty-five windows of each cell. Cohort A is the comparison's baseline argument
and cohort B its candidate; the assignment is arbitrary, since both measure the
same code.

| Cell | Arm | Cohort A ns/call | Cohort B ns/call | B/A | Verdict |
|---|---|---:|---:|---:|---|
| `bit_backend/and_inplace/words=1` | scalar | 3.731598 | 3.731451 | 0.999961 | PASS |
| `bit_backend/and_inplace/words=8` | simd | 4.403255 | 4.403818 | 1.000128 | PASS |
| `bit_backend/not_inplace/words=1` | scalar | 2.403617 | 2.405500 | 1.000784 | PASS |
| `bit_backend/not_inplace/words=8` | simd | 3.948830 | 3.949262 | 1.000109 | PASS |
| `bit_backend/or_inplace/words=1` | scalar | 3.520309 | 3.519959 | 0.999901 | PASS |
| `bit_backend/or_inplace/words=8` | simd | 4.840390 | 4.833791 | 0.998637 | PASS |
| `bit_backend/popcount/words=1` | scalar | 1.763466 | 1.763516 | 1.000028 | PASS |
| `bit_backend/popcount/words=8` | simd | 3.499331 | 3.499370 | 1.000011 | PASS |
| `bit_backend/xor_inplace/words=1` | scalar | 2.855176 | 2.855218 | 1.000015 | PASS |
| `bit_backend/xor_inplace/words=16` | simd | 4.022873 | 3.985336 | 0.990669 | PASS |
| `bit_backend/xor_inplace/words=4` | scalar | 3.740036 | 3.739931 | 0.999972 | PASS |
| `bit_backend/xor_inplace/words=64` | simd | 7.292857 | 7.431939 | 1.019071 | PASS |
| `bit_backend/xor_inplace/words=7` | scalar | 4.617880 | 4.615476 | 0.999479 | PASS |
| `bit_backend/xor_inplace/words=8` | simd | 3.742124 | 3.741955 | 0.999955 | PASS |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | horner | 14,276,110.847059 | 14,271,495.317647 | 0.999677 | PASS |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | horner | 57,203,784.640000 | 57,223,030.950000 | 1.000336 | PASS |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | subproduct | 25,986,063.088889 | 26,025,030.404444 | 1.001500 | PASS |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | horner | 14,270,650.352941 | 14,270,921.411765 | 1.000019 | PASS |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | horner | 57,140,591.550000 | 57,117,829.950000 | 0.999602 | PASS |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | subproduct | 18,639,851.452308 | 18,650,678.218462 | 1.000581 | PASS |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | schoolbook | 3,834,071.177160 | 3,833,374.777846 | 0.999818 | PASS |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | schoolbook | 5,184,462.820084 | 5,135,911.436515 | 0.990635 | PASS |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | newton | 2,337,892.807519 | 2,335,749.060413 | 0.999083 | PASS |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | newton | 5,138,202.803333 | 5,136,344.062500 | 0.999638 | PASS |
| `polynomial/mul/len=16` | schoolbook | 322.347721 | 322.659078 | 1.000966 | PASS |
| `polynomial/mul/len=256` | karatsuba | 36,572.606110 | 36,725.553545 | 1.004182 | PASS |
| `polynomial/mul/len=32` | schoolbook | 1,245.702499 | 1,245.678911 | 0.999981 | PASS |
| `polynomial/mul/len=33` | karatsuba | 1,147.451884 | 1,152.768042 | 1.004633 | PASS |
| `polynomial/mul/len=64` | karatsuba | 3,795.191339 | 3,838.980010 | 1.011538 | PASS |
| `polynomial/mul_fast/len=128` | ntt | 12,029.015426 | 12,040.326833 | 1.000940 | PASS |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,187.233924 | 1,186.994536 | 0.999798 | PASS |
| `polynomial/mul_fast/len=512` | ntt | 54,255.783222 | 54,224.848717 | 0.999430 | PASS |
| `polynomial/mul_fast/len=64` | mul_dispatch | 3,793.307825 | 3,817.549065 | 1.006391 | PASS |
| `polynomial/mul_fast/len=65` | ntt | 12,056.883642 | 12,062.019464 | 1.000426 | PASS |

Geometric mean of the thirty-four ratios: **1.000810**. `RESULT: PASS`.

## Dispersion

Within-execution dispersion is the minimum--maximum sample coefficient of
variation across the five repetitions of an execution; across-execution
dispersion is the sample coefficient of variation of the five execution-level
pooled rates. Each column reports the worse of the two cohorts, and the two
statistics are deliberately reported separately.

| Cell | Within-execution CV | Across-execution CV |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | 0.029--0.105% | 0.078% |
| `bit_backend/and_inplace/words=8` | 0.046--0.149% | 0.043% |
| `bit_backend/not_inplace/words=1` | 0.019--0.104% | 0.081% |
| `bit_backend/not_inplace/words=8` | 0.017--0.106% | 0.037% |
| `bit_backend/or_inplace/words=1` | 0.043--0.120% | 0.085% |
| `bit_backend/or_inplace/words=8` | 0.041--0.402% | 0.208% |
| `bit_backend/popcount/words=1` | 0.090--0.247% | 0.058% |
| `bit_backend/popcount/words=8` | 0.037--0.223% | 0.091% |
| `bit_backend/xor_inplace/words=1` | 0.045--0.236% | 0.063% |
| `bit_backend/xor_inplace/words=16` | 0.060--0.337% | 1.154% |
| `bit_backend/xor_inplace/words=4` | 0.008--0.095% | 0.035% |
| `bit_backend/xor_inplace/words=64` | 0.099--0.214% | 4.147% |
| `bit_backend/xor_inplace/words=7` | 0.060--0.314% | 0.106% |
| `bit_backend/xor_inplace/words=8` | 0.018--0.104% | 0.065% |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.047--0.125% | 0.064% |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.037--0.126% | 0.195% |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.088--0.324% | 0.165% |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.039--0.151% | 0.036% |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.062--0.139% | 0.030% |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.086--0.206% | 0.197% |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.036--0.112% | 0.045% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.025--0.132% | 2.620% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.040--0.117% | 0.128% |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.043--0.122% | 0.148% |
| `polynomial/mul/len=16` | 0.060--0.472% | 0.121% |
| `polynomial/mul/len=256` | 0.044--2.400% | 0.796% |
| `polynomial/mul/len=32` | 0.097--0.290% | 0.068% |
| `polynomial/mul/len=33` | 0.020--0.157% | 0.939% |
| `polynomial/mul/len=64` | 0.060--3.101% | 2.474% |
| `polynomial/mul_fast/len=128` | 0.023--0.108% | 0.153% |
| `polynomial/mul_fast/len=32` | 0.045--0.121% | 0.061% |
| `polynomial/mul_fast/len=512` | 0.031--0.098% | 0.308% |
| `polynomial/mul_fast/len=64` | 0.043--1.307% | 1.064% |
| `polynomial/mul_fast/len=65` | 0.030--0.092% | 0.100% |

Pooling five executions divides the across-execution dispersion by roughly
`sqrt(5)`, which is why three cells with across-execution dispersion above 2 %
still land inside 1.91 % in the comparison above. The worst across-execution
figure is 4.15 %, below the 5 % per-cell tolerance, so the plan's §7
noise-dominated clause is not triggered for any cell.

## Falsification record

### Uncontrolled fixture alignment, corrected before this run

An earlier pair of cohorts, taken with fixture buffers left at the allocator's
default alignment, failed the comparison at two of thirty-four cells:
`bit_backend/and_inplace/words=8` at 1.088884 and
`bit_backend/xor_inplace/words=16` at 1.133013, both far outside the 5 %
tolerance for code that had not changed.

The cause was not host noise. In the execution that produced the
`xor_inplace/words=16` outlier, that single cell ran at 9.09 ns/call against
5.07--5.24 ns/call in the four sibling executions, while every other bit cell in
the same process moved by at most 6 % and every polynomial cell by at most 0.4 %.
A whole-process effect such as a frequency change would have moved all of them.
The signature is per-cell and per-process, which is what an uncontrolled
allocation address produces: a 64-byte cache line holds eight `u64`, and a
32-byte AVX2 load that straddles two lines costs materially more.

The harness now lays each cell's eight fixture buffers out contiguously on
64-byte boundaries and asserts that before timing, and the plan's §3 records the
discipline. The two failing cells then measured 1.000128 and 0.990669. The
finding is recorded rather than rerun away: it is the reason the alignment
requirement is part of the protocol.

### `NTT_THRESHOLD` selects the more expensive arm at the crossover

Two adjacent pinned cells bracket `NTT_THRESHOLD` = 128 on `out_len`. At
`out_len` = 127 the `mul_fast` dispatcher takes the `FieldPoly::mul` arm and
costs 3,793 ns; at `out_len` = 129 it takes the NTT arm and costs 12,057 ns. The
step is 3.18x upward on crossing into the arm the threshold selects for being
cheaper, and the two arms only approach each other near `out_len` = 255, where
NTT costs 12,029 ns.

This contradicts the tuning claim carried by the constant's rustdoc at
`crates/gf2-core/src/field/poly.rs:2711`. It is recorded here and is out of scope
for issue `e8fe47f5`: changing the constant would move the pinned bracket and
re-pin this set. It belongs to the calibration sweep of issue `5ecc9bf8`, whose
selection rule is exactly the measurement this observation calls for.

The observation does not affect this set's validity as a non-regression gate.
Both arms are measured, at fixed sizes, before and after the cutover; whether the
threshold sits at the cost-optimal point is a separate question from whether the
cutover changes the cost of either arm.

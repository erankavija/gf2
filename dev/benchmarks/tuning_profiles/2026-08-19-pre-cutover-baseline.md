# Pre-cutover baseline receipt for the pinned selector non-regression set

This is the pre-cutover baseline of issue `278acf3a`: one execution of the
procedure of
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md) from
a clean revision, while the pilot selectors still read their compiled-in
constants. Issue `50b47eae` takes the post-cutover receipt, and the plan's §5
rule and §4 tolerance judge that receipt against this one.

The procedure runs unmodified. The plan, the harness, the pinned cell set of §2
and the tolerance of §4 stand as issue `e8fe47f5` pins them, and this run
changes nothing under `crates/`.

## Pre-cutover state

`select_backend_for_size` reads the compiled-in `SIMD_MIN_WORDS_DEFAULT` at
`crates/gf2-core/src/kernels/backend.rs:100`, defined at `:83`. The four
polynomial thresholds are read as constants at
`crates/gf2-core/src/field/poly.rs:2537` and `:2636` (`KARATSUBA_THRESHOLD`),
`:2874` (`NTT_THRESHOLD`), `:3210` (`DIV_REM_THRESHOLD`), and `:1402` and
`:3259` (`SUBPRODUCT_THRESHOLD`). Neither file names `crate::tuning`. The
tuning-profile mechanism of issue `f35daec0` is therefore present in the tree
and read by no selector, which is the state this baseline captures.

## Result

The run records 850 rows: thirty-four cells x five executions x five
repetitions. Every row carries `source_dirty=false` and revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6`. Every cell resolves to the arm the
plan's §2 predicts, and every cell's equivalence probe passes — a failed probe
aborts the run, and this run exits 0 with the full row count.

No cell's within-receipt dispersion reaches the plan's τ_cell of 5 %, so §7's
noise-dominated clause is triggered for no cell. The widest across-execution
dispersion is 4.668 % at `bit_backend/xor_inplace/words=64`, the same cell the
procedure-verification receipt records at 4.147 %.

A separate finding does contradict the noise assumption the tolerance rests on:
across two builds of behaviourally unchanged selector code, nine of the
thirty-four cells sit outside the ±5 % band. The Falsification record below
carries it with its numbers.

## Reproducible protocol and provenance

Every value below is observed during this run.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | [`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md), executed unmodified |
| Source revision | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`; every row records `source_dirty=false` |
| Bench binary SHA-256 | `acbfd9c49325b4063b7a419114e05bacd6b98299bbde45fc89016f0d162db0fe` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0 bench` |
| Features | `simd` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`, unheld at start; CPUs 6--11, observed as `Cpus_allowed_list: 6-11` inside the wrapper |
| Niceness | The wrapper's best-effort `nice -n -5` is denied to this non-root user; `nice` reports `cannot set niceness: Permission denied` and the child runs at niceness 0. The lock and the affinity remain in force |
| Timed work | Five fresh executions of five repetitions, `--target-ms 250` |
| Measured duration | 2026-08-19 18:01:49--18:05:36 UTC (3 min 47 s), the wrapper's whole child lifetime |
| Raw file | [`2026-08-19-pre-cutover-baseline.csv`](2026-08-19-pre-cutover-baseline.csv), SHA-256 `2672a911d99339648dda42abe91d7ec080c35981af164b31ccee275a5ed705cb` |

The bench binary is built before the timed run, so each execution reports
`Finished bench profile in 0.04s` and no compilation happens under the lock. The
raw file is written to `/tmp/gf2-e8fe47f5-pre-cutover-baseline-0c072d73.csv`,
checked absent beforehand, and copied byte-for-byte into this directory
afterwards; `cmp` reports the copy identical and both paths hash to the SHA-256
above.

## Step 0 — harness against the plan

`--self-check` prints:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

`per_cell_tolerance=0.050000` and `set_tolerance=0.020000` equal the plan's §4
values, and `cells=34` equals its §2 count.

`--list-cells` prints thirty-four rows, reproducing the plan's §2 table cell for
cell with the arm it predicts for each:

```
cell	family	arm	size_a	size_b
bit_backend/xor_inplace/words=1	bit_backend	scalar	1	0
bit_backend/xor_inplace/words=4	bit_backend	scalar	4	0
bit_backend/xor_inplace/words=7	bit_backend	scalar	7	0
bit_backend/xor_inplace/words=8	bit_backend	simd	8	0
bit_backend/xor_inplace/words=16	bit_backend	simd	16	0
bit_backend/xor_inplace/words=64	bit_backend	simd	64	0
bit_backend/and_inplace/words=1	bit_backend	scalar	1	0
bit_backend/and_inplace/words=8	bit_backend	simd	8	0
bit_backend/or_inplace/words=1	bit_backend	scalar	1	0
bit_backend/or_inplace/words=8	bit_backend	simd	8	0
bit_backend/not_inplace/words=1	bit_backend	scalar	1	0
bit_backend/not_inplace/words=8	bit_backend	simd	8	0
bit_backend/popcount/words=1	bit_backend	scalar	1	0
bit_backend/popcount/words=8	bit_backend	simd	8	0
polynomial/mul/len=16	polynomial	schoolbook	16	0
polynomial/mul/len=32	polynomial	schoolbook	32	0
polynomial/mul/len=33	polynomial	karatsuba	33	0
polynomial/mul/len=64	polynomial	karatsuba	64	0
polynomial/mul/len=256	polynomial	karatsuba	256	0
polynomial/mul_fast/len=32	polynomial	mul_dispatch	32	0
polynomial/mul_fast/len=64	polynomial	mul_dispatch	64	0
polynomial/mul_fast/len=65	polynomial	ntt	65	0
polynomial/mul_fast/len=128	polynomial	ntt	128	0
polynomial/mul_fast/len=512	polynomial	ntt	512	0
polynomial/div_rem_auto/dividend=4096/divisor=1024	polynomial	schoolbook	4096	1024
polynomial/div_rem_auto/dividend=4096/divisor=2047	polynomial	schoolbook	4096	2047
polynomial/div_rem_auto/dividend=4096/divisor=2048	polynomial	newton	4096	2048
polynomial/div_rem_auto/dividend=8192/divisor=4096	polynomial	newton	8192	4096
polynomial/batch_evaluate/coeffs=2048/points=2048	polynomial	horner	2048	2048
polynomial/batch_evaluate/coeffs=4095/points=4095	polynomial	horner	4095	4095
polynomial/batch_evaluate/coeffs=4096/points=4096	polynomial	subproduct	4096	4096
polynomial/batch_evaluate_auto/coeffs=2048/points=2048	polynomial	horner	2048	2048
polynomial/batch_evaluate_auto/coeffs=4095/points=4095	polynomial	horner	4095	4095
polynomial/batch_evaluate_auto/coeffs=4096/points=4096	polynomial	subproduct	4096	4096
```

## Step 1 — commands

```sh
OUT=/tmp/gf2-e8fe47f5-pre-cutover-baseline-0c072d73.csv
test ! -e "$OUT"
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash -lc '
  for e in 1 2 3 4 5; do
    cargo +1.95.0 bench -p gf2-core --features simd \
      --bench selector_non_regression -- \
      --execution "$e" --repetitions 5 --target-ms 250 \
      --output "'"$OUT"'" --append
  done'
```

## Baseline rates

Pooled nanoseconds per call per the plan's §5: the sum of `elapsed_ns` over all
twenty-five windows of a cell divided by the sum of `calls`. Every figure below
is computed from the committed raw file.

| Cell | Arm | Pooled ns/call |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | scalar | 3.519421 |
| `bit_backend/and_inplace/words=8` | simd | 4.611779 |
| `bit_backend/not_inplace/words=1` | scalar | 2.833877 |
| `bit_backend/not_inplace/words=8` | simd | 4.159037 |
| `bit_backend/or_inplace/words=1` | scalar | 4.157592 |
| `bit_backend/or_inplace/words=8` | simd | 4.651777 |
| `bit_backend/popcount/words=1` | scalar | 1.853240 |
| `bit_backend/popcount/words=8` | simd | 3.501248 |
| `bit_backend/xor_inplace/words=1` | scalar | 3.282865 |
| `bit_backend/xor_inplace/words=16` | simd | 4.237336 |
| `bit_backend/xor_inplace/words=4` | scalar | 3.945581 |
| `bit_backend/xor_inplace/words=64` | simd | 7.510168 |
| `bit_backend/xor_inplace/words=7` | scalar | 4.512804 |
| `bit_backend/xor_inplace/words=8` | simd | 3.785692 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | horner | 14,259,449.788235 |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | horner | 57,113,481.640000 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | subproduct | 27,445,793.431818 |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | horner | 14,261,197.134118 |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | horner | 56,938,298.770000 |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | subproduct | 19,245,019.569231 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | schoolbook | 4,312,979.932867 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | schoolbook | 5,290,880.676068 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | newton | 2,301,365.583704 |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | newton | 5,091,288.531148 |
| `polynomial/mul/len=16` | schoolbook | 316.335008 |
| `polynomial/mul/len=256` | karatsuba | 36,942.861779 |
| `polynomial/mul/len=32` | schoolbook | 1,222.719206 |
| `polynomial/mul/len=33` | karatsuba | 1,140.814861 |
| `polynomial/mul/len=64` | karatsuba | 3,807.325330 |
| `polynomial/mul_fast/len=128` | ntt | 11,919.874440 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,190.350051 |
| `polynomial/mul_fast/len=512` | ntt | 53,574.708467 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 3,813.538569 |
| `polynomial/mul_fast/len=65` | ntt | 11,910.678494 |

## Per-cell dispersion

Two statistics, as the procedure-verification receipt defines them.
Within-execution dispersion is the minimum--maximum sample coefficient of
variation across the five repetitions of an execution; across-execution
dispersion is the sample coefficient of variation of the five execution-level
pooled rates. The plan's §7 judges a cell's own dispersion against τ_cell = 5 %.

| Cell | Within-execution CV | Across-execution CV |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | 0.027--0.061% | 0.132% |
| `bit_backend/and_inplace/words=8` | 0.035--0.059% | 0.049% |
| `bit_backend/not_inplace/words=1` | 0.022--0.065% | 0.074% |
| `bit_backend/not_inplace/words=8` | 0.046--0.129% | 0.148% |
| `bit_backend/or_inplace/words=1` | 0.035--0.056% | 0.148% |
| `bit_backend/or_inplace/words=8` | 0.067--0.191% | 0.163% |
| `bit_backend/popcount/words=1` | 0.097--0.231% | 0.135% |
| `bit_backend/popcount/words=8` | 0.039--0.063% | 0.094% |
| `bit_backend/xor_inplace/words=1` | 0.034--0.205% | 0.172% |
| `bit_backend/xor_inplace/words=16` | 0.019--0.083% | 2.119% |
| `bit_backend/xor_inplace/words=4` | 0.037--0.067% | 0.091% |
| `bit_backend/xor_inplace/words=64` | 0.083--0.270% | 4.668% |
| `bit_backend/xor_inplace/words=7` | 0.161--0.522% | 0.210% |
| `bit_backend/xor_inplace/words=8` | 0.035--0.054% | 2.482% |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.050--0.105% | 0.089% |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.052--0.235% | 0.275% |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.109--0.525% | 0.098% |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.050--0.123% | 0.079% |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.041--0.078% | 0.354% |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.469--1.139% | 0.724% |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.048--0.096% | 0.089% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.069--0.317% | 1.452% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.069--0.138% | 0.164% |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.032--0.095% | 0.110% |
| `polynomial/mul/len=16` | 0.144--0.506% | 0.219% |
| `polynomial/mul/len=256` | 0.024--0.161% | 0.828% |
| `polynomial/mul/len=32` | 0.096--0.146% | 0.065% |
| `polynomial/mul/len=33` | 0.040--0.086% | 0.057% |
| `polynomial/mul/len=64` | 0.030--0.087% | 0.058% |
| `polynomial/mul_fast/len=128` | 0.052--0.187% | 0.242% |
| `polynomial/mul_fast/len=32` | 0.043--0.086% | 0.049% |
| `polynomial/mul_fast/len=512` | 0.043--0.069% | 0.290% |
| `polynomial/mul_fast/len=64` | 0.040--0.065% | 0.077% |
| `polynomial/mul_fast/len=65` | 0.045--0.102% | 0.057% |

No cell reaches τ_cell on either statistic, so no cell is noise-dominated in
this receipt. The three largest across-execution figures are 4.668 % at
`bit_backend/xor_inplace/words=64`, 2.482 % at `bit_backend/xor_inplace/words=8`
and 2.119 % at `bit_backend/xor_inplace/words=16`; the largest within-execution
figure is 1.139 % at
`polynomial/batch_evaluate_auto/coeffs=4096/points=4096`. Pooling five
executions divides the across-execution dispersion by roughly `sqrt(5)`, so the
pooled statistic of §5 carries less spread than the table above shows.

## Comparison eligibility

The plan's §5 comparison mode refuses two receipts whose schema tokens, cell
sets, or per-cell identities disagree, reporting
`RESULT: FAIL (selector identity mismatch)` and printing no per-cell verdict.
Running this baseline against the committed cohort A proves it is eligible: the
run prints all thirty-four per-cell lines and a geometric mean, so schema, cell
set and every cell's arm, family and operand sizes agree.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-19-procedure-verification-cohort-a.csv
```

**This is an eligibility check, not a regression verdict.** The cohort is a
different run of pre-cutover code from a different build, so the plan defines no
verdict for the pair; `50b47eae`'s comparison against the post-cutover receipt
is the real one. The tolerance-class `RESULT: FAIL` the run reports, and its
exit status 1, belong to the Falsification record below.

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv candidate=dev/benchmarks/tuning_profiles/2026-08-19-procedure-verification-cohort-a.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.519421 3.731598 1.060287 FAIL
bit_backend/and_inplace/words=8 simd 4.611779 4.403255 0.954784 PASS
bit_backend/not_inplace/words=1 scalar 2.833877 2.403617 0.848172 PASS
bit_backend/not_inplace/words=8 simd 4.159037 3.948830 0.949458 PASS
bit_backend/or_inplace/words=1 scalar 4.157592 3.520309 0.846718 PASS
bit_backend/or_inplace/words=8 simd 4.651777 4.840390 1.040546 PASS
bit_backend/popcount/words=1 scalar 1.853240 1.763466 0.951558 PASS
bit_backend/popcount/words=8 simd 3.501248 3.499331 0.999453 PASS
bit_backend/xor_inplace/words=1 scalar 3.282865 2.855176 0.869721 PASS
bit_backend/xor_inplace/words=16 simd 4.237336 4.022873 0.949387 PASS
bit_backend/xor_inplace/words=4 scalar 3.945581 3.740036 0.947905 PASS
bit_backend/xor_inplace/words=64 simd 7.510168 7.292857 0.971064 PASS
bit_backend/xor_inplace/words=7 scalar 4.512804 4.617880 1.023284 PASS
bit_backend/xor_inplace/words=8 simd 3.785692 3.742124 0.988491 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14259449.788235 14276110.847059 1.001168 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57113481.640000 57203784.640000 1.001581 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 27445793.431818 25986063.088889 0.946814 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14261197.134118 14270650.352941 1.000663 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 56938298.770000 57140591.550000 1.003553 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 19245019.569231 18639851.452308 0.968555 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4312979.932867 3834071.177160 0.888961 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5290880.676068 5184462.820084 0.979887 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2301365.583704 2337892.807519 1.015872 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5091288.531148 5138202.803333 1.009215 PASS
polynomial/mul/len=16 schoolbook 316.335008 322.347721 1.019007 PASS
polynomial/mul/len=256 karatsuba 36942.861779 36572.606110 0.989978 PASS
polynomial/mul/len=32 schoolbook 1222.719206 1245.702499 1.018797 PASS
polynomial/mul/len=33 karatsuba 1140.814861 1147.451884 1.005818 PASS
polynomial/mul/len=64 karatsuba 3807.325330 3795.191339 0.996813 PASS
polynomial/mul_fast/len=128 ntt 11919.874440 12029.015426 1.009156 PASS
polynomial/mul_fast/len=32 mul_dispatch 1190.350051 1187.233924 0.997382 PASS
polynomial/mul_fast/len=512 ntt 53574.708467 54255.783222 1.012713 PASS
polynomial/mul_fast/len=64 mul_dispatch 3813.538569 3793.307825 0.994695 PASS
polynomial/mul_fast/len=65 ntt 11910.678494 12056.883642 1.012275 PASS
geometric_mean 0.977297 PASS
RESULT: FAIL
```

The comparison is one-sided: it fails a cell whose candidate rate exceeds
1.05x the baseline's, so the single `FAIL` verdict above is the one cell where
the cohort is slower than this baseline. The nine cells that differ by more than
5 % in either direction are the finding below.

## Falsification record

### Two builds of unchanged selector code differ by up to 18 % at the pinned cells

The plan's §4 grounds τ_cell = 5 % on the dispersion of the two
procedure-verification cohorts, whose widest per-cell deviation is 1.91 %. Those
cohorts run the same bench binary back to back inside one lock session. This
baseline runs a different binary — revision `0c072d73` against the cohorts'
`1d8a289a` — on the same host under the same wrapper, affinity, governor,
toolchain and protocol, and against it nine of the thirty-four cells fall
outside ±5 %:

| Cell | Arm | Cohort A / baseline | Cohort B / baseline |
|---|---|---:|---:|
| `bit_backend/or_inplace/words=1` | scalar | 0.846718 | 0.846634 |
| `bit_backend/not_inplace/words=1` | scalar | 0.848172 | 0.848837 |
| `bit_backend/xor_inplace/words=1` | scalar | 0.869721 | 0.869734 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | schoolbook | 0.888961 | 0.888800 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | subproduct | 0.946814 | 0.948234 |
| `bit_backend/xor_inplace/words=4` | scalar | 0.947905 | 0.947878 |
| `bit_backend/xor_inplace/words=16` | simd | 0.949387 | 0.940529 |
| `bit_backend/not_inplace/words=8` | simd | 0.949458 | 0.949562 |
| `bit_backend/and_inplace/words=1` | scalar | 1.060287 | 1.060245 |

The widest is `bit_backend/or_inplace/words=1`, where this baseline's 4.157592
ns/call stands 18.1 % above cohort A's 3.520309, with cohort B agreeing to
0.01 %. The geometric mean of the thirty-four ratios is 0.977297 against cohort
A and 0.978088 against cohort B.

The effect is a property of the binary rather than of the host at run time. The
shifts run in both directions — `and_inplace/words=1` is 6.0 % the other way
from its three scalar siblings — and the two cohorts reproduce each ratio to
within 0.1 % on seven of the nine cells, while every cell's own
across-execution dispersion in this receipt stays under 4.67 %. A whole-host
effect such as a frequency or thermal change moves every cell in one direction
within a run; a stable per-cell split that both cohorts agree on does not come
from run-time host state. The two of the nine where the cohorts disagree by
more than 0.1 % are `bit_backend/xor_inplace/words=16` at 0.93 % and
`polynomial/batch_evaluate/coeffs=4096/points=4096` at 0.15 %. Over all
thirty-four cells the widest cohort-to-cohort disagreement is 1.9 % at
`bit_backend/xor_inplace/words=64` (0.971064 against 0.989584), the cell both
receipts record with the highest across-execution dispersion.

The selector logic is identical across the two revisions: the only change to
`select_backend_for_size` between them lifts its function-local `_SIMD_THRESHOLD
= 8` to the module-level `SIMD_MIN_WORDS_DEFAULT = 8`, and the four polynomial
thresholds are untouched. The revisions differ by the tuning-profile module of
`f35daec0`, which no selector reads, and by the harness commits of `e8fe47f5`.
This measurement establishes that the shift tracks the binary; it does not
isolate which change in the binary produces it. The one placement mechanism the
plan already names is excluded: both runs use the harness that lays each
bit-logical cell's fixture banks on 64-byte boundaries and asserts it before
timing, per §3, so the fixture alignment §3 records is identical on both sides.

**Why this contradicts the plan's noise assumption.** §4 reads the cohorts'
1.91 % as the spread τ_cell must absorb, giving the tolerance a factor of 2.6.
The comparison τ_cell actually gates — this baseline against `50b47eae`'s
post-cutover receipt — is necessarily across two builds, and the across-build
component measured here reaches 18.1 %, which is 3.6 times τ_cell. A cutover
that adds no cost can therefore trip the per-cell rule at these cells, and one
that adds a real cost can be masked at them.

Per §7 and `@/inv/falsification-preserved` this is recorded rather than
accommodated: the tolerance stays at its predeclared values, the pinned set
keeps all thirty-four cells, and this run stands as taken rather than being
repeated until it agrees. The finding bears on how `50b47eae`'s comparison is
read, which is a lead decision rather than one this receipt takes.

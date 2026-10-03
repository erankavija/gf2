# Post-cutover receipt for the pinned selector non-regression set

This is the post-cutover receipt of issue `50b47eae`: one execution of the
procedure of
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md) from
a clean revision, after the pilot selectors read `tuning::active()`, together
with the across-build control arm that issue `51058f8e` predeclares in
[`across-build-control-arm-v1.md`](across-build-control-arm-v1.md). Both arms
run in one lock session.

The procedure runs unmodified. The plan, the harness, the pinned cell set of §2,
the tolerance of §4 and the comparison rule of §5 stand as issue `e8fe47f5`
pins them, and this run changes nothing under `crates/`.

**The comparison fails.** The plan's §5 rule applied to
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md)
against this receipt reports `RESULT: FAIL`: two of the thirty-four cells exceed
τ_cell = 5 %. The Falsification record below carries both cells with their
pooled rates, their ratios and the contradiction they raise, and the tolerance
stands at its predeclared value. Per plan §7 and control-arm §4.5 this run is
not repeated until it agrees.

## Post-cutover state

`select_backend_for_size` reads the active profile's
`bit_backend.simd_min_words` at `crates/gf2-core/src/kernels/backend.rs:105`;
the definition site of the conservative default it resolves to is the
compiled-in `SIMD_MIN_WORDS_DEFAULT` at `:83`. The five pinned bit-logical entry
points reach it through `crates/gf2-core/src/kernels/ops.rs`: `xor_inplace` at
`:93`, `and_inplace` at `:117`, `or_inplace` at `:154`, `not_inplace` at `:182`,
and `popcount` at `:204`.

The four polynomial thresholds resolve through the same mechanism in
`crates/gf2-core/src/field/poly.rs`: `karatsuba_min_degree` at `:2178` and
`:2716`, `subproduct_min_len` at `:2248` and `:2262`, `karatsuba_max_out_len` at
`:2814`, and `div_rem_fast_min_len` at `:3038`.

This run installs no tuning profile, and no profile is installed by any code the
harness reaches: `gf2_core::tuning::active` resolves the process-wide `OnceLock`
to `TuningProfile::CONSERVATIVE` when no caller installs one first, without
consulting the filesystem or the environment. Every resolved value therefore
equals the pre-cutover constant, and what this receipt measures is the cost of
the profile read on the selection path rather than a change of selected arm.
Every cell resolves to the arm the plan's §2 predicts, in both arms.

## Result

Each arm records 850 rows: thirty-four cells x five executions x five
repetitions. Every row of the post-cutover arm carries `source_dirty=false` and
revision `e2e2ced78a1d544c9dd4f545a58d5bcf1e751be2`; every row of the control arm
carries `source_dirty=false` and revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6`. Every cell's equivalence probe
passes in both arms — a failed probe aborts the run, and both arms exit 0 with
the full row count.

The plan's §5 comparison of the baseline against the post-cutover arm reports:

- **τ_set holds.** The geometric mean of the thirty-four per-cell ratios is
  0.998853, below the predeclared 1.02.
- **τ_cell fails at two cells.** `bit_backend/popcount/words=1` rises from
  1.853240 to 2.191107 ns/call, a ratio of 1.182311, and
  `bit_backend/popcount/words=8` rises from 3.501248 to 3.726078 ns/call, a
  ratio of 1.064214. Both exceed 1.05.
- The rule requires both to hold, so the comparison is `RESULT: FAIL` and exits
  non-zero.

No cell's own dispersion within either arm reaches τ_cell, so §7's
noise-dominated clause is triggered for no cell and neither excursion is
attributable to within-receipt noise. The widest across-execution dispersion is
2.726 % at `bit_backend/and_inplace/words=1` in the post-cutover arm and 4.358 %
at `bit_backend/popcount/words=1` in the control arm.

## Reproducible protocol and provenance

Every value below is observed during this run. The two arms share one host
state, one lock, one affinity mask and one wrapper invocation; the rows that
differ between them are marked.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | [`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md), executed unmodified; control arm per [`across-build-control-arm-v1.md`](across-build-control-arm-v1.md) §2 |
| Source revision, post-cutover arm | `e2e2ced78a1d544c9dd4f545a58d5bcf1e751be2`; every row records `source_dirty=false` |
| Source revision, control arm | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the baseline receipt records; every row records `source_dirty=false` |
| Bench binary SHA-256, post-cutover arm | `f581f8681d68ec32b3a4ad073df648bcd0e9f67dab4a51401a4fc7fff609a0d2` |
| Bench binary SHA-256, control arm | `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710` |
| Control-arm checkout | `.agents/worktrees/control-0c072d73`, detached at `0c072d73ca65cf50af98b8c4b61ed876f8218df6`; its harness file is byte-identical to the post-cutover checkout's, and the four public polynomial thresholds hold the same values in both |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0 bench`, both arms |
| Features | `simd`, both arms |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`, one invocation holding `/tmp/gf2-ccx1.lock` for both arms; the lock is unheld at start and acquired without blocking, and `/proc/locks` inside the wrapper records `FLOCK ADVISORY WRITE` held by the wrapper's own PID on the lock file's device and inode. CPUs 6--11, observed as `Cpus_allowed_list: 6-11` inside the wrapper |
| Niceness | The wrapper's best-effort `nice -n -5` is denied to this non-root user; `nice` reports `cannot set niceness: Permission denied` and the child runs at niceness 0. The lock and the affinity remain in force |
| Host quiescence | No `permanent-campaign-runner` process, matched against `ps -eo pid,comm,args`. The one-minute load average sits in the 0.13--0.27 band over the minutes before the run and reads 0.16 as the wrapper is invoked; the five- and fifteen-minute averages are 1.18 and 6.73, decaying from the two bench builds that precede the session |
| Timed work | Five fresh executions of five repetitions per arm, `--target-ms 250` |
| Measured duration | 2026-08-19 23:53:17--2026-08-20 00:00:46 UTC (7 min 29 s), the wrapper's whole child lifetime, covering both arms |
| Raw file, post-cutover arm | [`2026-08-20-post-cutover-receipt.csv`](2026-08-20-post-cutover-receipt.csv), SHA-256 `9c38f552b59e07af06e1c313baf80892e96c979c341fd9aeacb623fef8661f51` |
| Raw file, control arm | [`2026-08-20-post-cutover-control-arm.csv`](2026-08-20-post-cutover-control-arm.csv), SHA-256 `612ff0318dcdd855feefced4419d0b14219438e4a672235b9df243d943ba2405` |

Both bench binaries are built before the timed run, so each execution reports
`Finished bench profile in 0.04s` to `0.06s` and no compilation happens under
the lock. The raw files are written to
`/tmp/gf2-e8fe47f5-post-cutover-e2e2ced7.csv` and
`/tmp/gf2-e8fe47f5-control-arm-0c072d73.csv`, both checked absent beforehand,
and copied byte-for-byte into this directory afterwards; `cmp` reports each copy
identical and each pair of paths hashes to the SHA-256 above.

## Arm ordering

The ten executions of the session interleave the two arms in the counterbalanced
order **C P P C C P P C C P**, where C is the control arm and P is the
post-cutover arm:

| Slot | Arm | Execution | Start (UTC) |
|---:|---|---:|---|
| 1 | control | 1 | 23:53:18 |
| 2 | post-cutover | 1 | 23:54:02 |
| 3 | post-cutover | 2 | 23:54:47 |
| 4 | control | 2 | 23:55:32 |
| 5 | control | 3 | 23:56:17 |
| 6 | post-cutover | 3 | 23:57:02 |
| 7 | post-cutover | 4 | 23:57:47 |
| 8 | control | 4 | 23:58:32 |
| 9 | control | 5 | 23:59:17 |
| 10 | post-cutover | 5 | 00:00:01 |

Running each arm as one contiguous block would confound arm with session half,
so a warm-up or thermal trend over the session would land entirely on one arm.
Interleaving in ABBA blocks removes that confound: each arm holds two of the
first four slots and two of the last four. Five executions per arm cannot
balance the mean slot position exactly — the ten positions sum to 55, so an
equal split would need 27.5 each — and the residual is one slot, with the
control arm at mean position 5.4 and the post-cutover arm at 5.6. The
assignment of that single slot is fixed before any number exists.

The residual matters only to the extent the session drifts, and it does not.
Taking each execution's set-level geometric mean against its own cell's
five-execution mean, the post-cutover arm reads 1.000014, 0.998798, 0.999439,
1.000321 and 1.001359 across executions 1--5, and the control arm reads
0.998886, 1.003731, 0.998804, 0.998438 and 0.999964. Both stay inside ±0.4 %
with no monotone trend, more than an order of magnitude below the smaller of the
two excursions the comparison reports and well inside τ_set.

## Step 0 — harness against the plan

`--self-check` prints the same line in both checkouts:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

`per_cell_tolerance=0.050000` and `set_tolerance=0.020000` equal the plan's §4
values, and `cells=34` equals its §2 count. `simd_min_words=8` is the value the
post-cutover checkout resolves from the active profile and the value the control
checkout reads from its compiled-in constant, so the two agree on the guard the
bit-backend cells straddle.

`--list-cells` prints thirty-four rows in each checkout, and the two listings are
byte-identical, reproducing the plan's §2 table cell for cell with the arm it
predicts for each:

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

Both arms run from one wrapper invocation, so one `flock -x` is held for the
whole session and both arms see one host state, as control-arm §2 requires. The
wrapper's child is a single script that alternates the two checkouts in the
order recorded above.

```sh
test ! -e /tmp/gf2-e8fe47f5-post-cutover-e2e2ced7.csv
test ! -e /tmp/gf2-e8fe47f5-control-arm-0c072d73.csv
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash -l run-both-arms.sh
```

`run-both-arms.sh` is the session driver. It records the host facts this receipt
cites, then runs the ten executions:

```sh
#!/usr/bin/env bash
set -euo pipefail

MAIN=/home/vkaskivuo/Projects/gf2
CTRL_TREE=/home/vkaskivuo/Projects/gf2/.agents/worktrees/control-0c072d73
POST_OUT=/tmp/gf2-e8fe47f5-post-cutover-e2e2ced7.csv
CTRL_OUT=/tmp/gf2-e8fe47f5-control-arm-0c072d73.csv

echo "child-start-utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "cpus-allowed: $(awk '/Cpus_allowed_list/{print $2}' /proc/self/status)"
echo "niceness: $(ps -o ni= -p $$ | tr -d ' ')"
echo "lock-inode: $(stat -c '%d:%i' /tmp/gf2-ccx1.lock)"
echo "proc-locks-flock:"
grep -i flock /proc/locks || true
echo "loadavg-at-start: $(cat /proc/loadavg)"

run_post() {
  cd "$MAIN"
  cargo +1.95.0 bench -p gf2-core --features simd \
    --bench selector_non_regression -- \
    --execution "$1" --repetitions 5 --target-ms 250 \
    --output "$POST_OUT" --append
}

run_ctrl() {
  cd "$CTRL_TREE"
  cargo +1.95.0 bench -p gf2-core --features simd \
    --bench selector_non_regression -- \
    --execution "$1" --repetitions 5 --target-ms 250 \
    --output "$CTRL_OUT" --append
}

# Counterbalanced interleave: C P P C C P P C C P
for slot in "1:C:1" "2:P:1" "3:P:2" "4:C:2" "5:C:3" "6:P:3" "7:P:4" "8:C:4" "9:C:5" "10:P:5"; do
  n="${slot%%:*}"; rest="${slot#*:}"; arm="${rest%%:*}"; exec_idx="${rest#*:}"
  echo "slot=$n arm=$arm execution=$exec_idx utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  if [ "$arm" = "P" ]; then run_post "$exec_idx"; else run_ctrl "$exec_idx"; fi
done

echo "child-end-utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "loadavg-at-end: $(cat /proc/loadavg)"
```

Each arm therefore runs the plan's §3 protocol unmodified: five fresh
executions of five repetitions at `--target-ms 250`, one process per execution,
under the MSRV toolchain with the `simd` feature and no tuning profile
installed. The script lives outside the repository, so no in-repository file is
created while the run is in flight and every recorded row reads
`source_dirty=false`.

## Post-cutover rates

Pooled nanoseconds per call per the plan's §5: the sum of `elapsed_ns` over all
twenty-five windows of a cell divided by the sum of `calls`. Every figure below
is computed from the committed raw file.

| Cell | Arm | Pooled ns/call |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | scalar | 3.627451 |
| `bit_backend/and_inplace/words=8` | simd | 4.616206 |
| `bit_backend/not_inplace/words=1` | scalar | 2.413072 |
| `bit_backend/not_inplace/words=8` | simd | 4.170461 |
| `bit_backend/or_inplace/words=1` | scalar | 3.741424 |
| `bit_backend/or_inplace/words=8` | simd | 4.641509 |
| `bit_backend/popcount/words=1` | scalar | 2.191107 |
| `bit_backend/popcount/words=8` | simd | 3.726078 |
| `bit_backend/xor_inplace/words=1` | scalar | 3.073433 |
| `bit_backend/xor_inplace/words=16` | simd | 4.415779 |
| `bit_backend/xor_inplace/words=4` | scalar | 3.958083 |
| `bit_backend/xor_inplace/words=64` | simd | 7.544440 |
| `bit_backend/xor_inplace/words=7` | scalar | 4.676144 |
| `bit_backend/xor_inplace/words=8` | simd | 3.962550 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | horner | 14,304,732.581176 |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | horner | 57,152,457.950000 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | subproduct | 26,447,142.782222 |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | horner | 14,298,439.341176 |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | horner | 56,856,947.600000 |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | subproduct | 18,727,633.341538 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | schoolbook | 4,071,227.625574 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | schoolbook | 5,308,643.563948 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | newton | 2,313,951.213035 |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | newton | 5,107,228.951037 |
| `polynomial/mul/len=16` | schoolbook | 319.984906 |
| `polynomial/mul/len=256` | karatsuba | 36,653.690122 |
| `polynomial/mul/len=32` | schoolbook | 1,222.746971 |
| `polynomial/mul/len=33` | karatsuba | 1,132.854865 |
| `polynomial/mul/len=64` | karatsuba | 3,798.080949 |
| `polynomial/mul_fast/len=128` | ntt | 11,959.599761 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,193.774262 |
| `polynomial/mul_fast/len=512` | ntt | 53,800.935222 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 3,802.020198 |
| `polynomial/mul_fast/len=65` | ntt | 11,969.733196 |

## Comparison against the baseline — the verdict

This is the comparison the plan's §5 defines and the epic gates on: the
pre-cutover baseline of `278acf3a` against this receipt, on pooled ns/call per
cell, judged by τ_cell = 5 % and τ_set = 2 %.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt.csv
```

The run prints all thirty-four per-cell lines and a geometric mean, so the §5
preconditions hold: both receipts carry schema `selector-non-regression-v1`,
their cell sets are identical and equal to the pinned set of §2, and every
cell's arm, family and operand sizes agree. The failure is therefore a tolerance
failure rather than a `RESULT: FAIL (selector identity mismatch)`.

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv candidate=dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.519421 3.627451 1.030695 PASS
bit_backend/and_inplace/words=8 simd 4.611779 4.616206 1.000960 PASS
bit_backend/not_inplace/words=1 scalar 2.833877 2.413072 0.851509 PASS
bit_backend/not_inplace/words=8 simd 4.159037 4.170461 1.002747 PASS
bit_backend/or_inplace/words=1 scalar 4.157592 3.741424 0.899902 PASS
bit_backend/or_inplace/words=8 simd 4.651777 4.641509 0.997793 PASS
bit_backend/popcount/words=1 scalar 1.853240 2.191107 1.182311 FAIL
bit_backend/popcount/words=8 simd 3.501248 3.726078 1.064214 FAIL
bit_backend/xor_inplace/words=1 scalar 3.282865 3.073433 0.936205 PASS
bit_backend/xor_inplace/words=16 simd 4.237336 4.415779 1.042112 PASS
bit_backend/xor_inplace/words=4 scalar 3.945581 3.958083 1.003169 PASS
bit_backend/xor_inplace/words=64 simd 7.510168 7.544440 1.004563 PASS
bit_backend/xor_inplace/words=7 scalar 4.512804 4.676144 1.036195 PASS
bit_backend/xor_inplace/words=8 simd 3.785692 3.962550 1.046717 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14259449.788235 14304732.581176 1.003176 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57113481.640000 57152457.950000 1.000682 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 27445793.431818 26447142.782222 0.963614 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14261197.134118 14298439.341176 1.002611 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 56938298.770000 56856947.600000 0.998571 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 19245019.569231 18727633.341538 0.973116 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4312979.932867 4071227.625574 0.943948 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5290880.676068 5308643.563948 1.003357 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2301365.583704 2313951.213035 1.005469 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5091288.531148 5107228.951037 1.003131 PASS
polynomial/mul/len=16 schoolbook 316.335008 319.984906 1.011538 PASS
polynomial/mul/len=256 karatsuba 36942.861779 36653.690122 0.992172 PASS
polynomial/mul/len=32 schoolbook 1222.719206 1222.746971 1.000023 PASS
polynomial/mul/len=33 karatsuba 1140.814861 1132.854865 0.993023 PASS
polynomial/mul/len=64 karatsuba 3807.325330 3798.080949 0.997572 PASS
polynomial/mul_fast/len=128 ntt 11919.874440 11959.599761 1.003333 PASS
polynomial/mul_fast/len=32 mul_dispatch 1190.350051 1193.774262 1.002877 PASS
polynomial/mul_fast/len=512 ntt 53574.708467 53800.935222 1.004223 PASS
polynomial/mul_fast/len=64 mul_dispatch 3813.538569 3802.020198 0.996980 PASS
polynomial/mul_fast/len=65 ntt 11910.678494 11969.733196 1.004958 PASS
geometric_mean 0.998853 PASS
RESULT: FAIL```

The comparison exits 1.

## Per-cell dispersion

Two statistics, as the procedure-verification receipt defines them.
Within-execution dispersion is the minimum--maximum sample coefficient of
variation across the five repetitions of an execution; across-execution
dispersion is the sample coefficient of variation of the five execution-level
pooled rates. The plan's §7 judges a cell's own dispersion against τ_cell = 5 %.

| Cell | Within-execution CV | Across-execution CV |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | 0.082--1.337% | 2.726% |
| `bit_backend/and_inplace/words=8` | 0.064--0.115% | 0.089% |
| `bit_backend/not_inplace/words=1` | 0.053--0.149% | 0.030% |
| `bit_backend/not_inplace/words=8` | 0.032--0.161% | 0.140% |
| `bit_backend/or_inplace/words=1` | 0.040--0.157% | 0.021% |
| `bit_backend/or_inplace/words=8` | 0.016--0.096% | 0.090% |
| `bit_backend/popcount/words=1` | 0.046--0.602% | 0.492% |
| `bit_backend/popcount/words=8` | 0.032--0.116% | 0.076% |
| `bit_backend/xor_inplace/words=1` | 0.044--0.109% | 0.083% |
| `bit_backend/xor_inplace/words=16` | 0.040--0.195% | 0.057% |
| `bit_backend/xor_inplace/words=4` | 0.045--0.373% | 0.222% |
| `bit_backend/xor_inplace/words=64` | 0.330--0.583% | 1.330% |
| `bit_backend/xor_inplace/words=7` | 0.119--0.435% | 0.132% |
| `bit_backend/xor_inplace/words=8` | 0.048--0.134% | 0.111% |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.088--0.157% | 0.088% |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.037--0.128% | 0.304% |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.102--0.337% | 0.131% |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.079--0.127% | 0.055% |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.053--0.191% | 0.009% |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.072--0.243% | 0.080% |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.038--0.165% | 0.065% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.034--0.131% | 1.379% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.029--0.285% | 0.092% |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.050--0.057% | 0.054% |
| `polynomial/mul/len=16` | 0.126--0.506% | 0.215% |
| `polynomial/mul/len=256` | 0.017--0.131% | 0.574% |
| `polynomial/mul/len=32` | 0.076--0.138% | 0.061% |
| `polynomial/mul/len=33` | 0.053--0.194% | 0.180% |
| `polynomial/mul/len=64` | 0.034--0.104% | 0.057% |
| `polynomial/mul_fast/len=128` | 0.024--0.321% | 0.141% |
| `polynomial/mul_fast/len=32` | 0.050--0.153% | 0.177% |
| `polynomial/mul_fast/len=512` | 0.054--0.122% | 0.094% |
| `polynomial/mul_fast/len=64` | 0.027--0.114% | 0.068% |
| `polynomial/mul_fast/len=65` | 0.033--0.114% | 0.071% |

No cell reaches τ_cell on either statistic, so no cell is noise-dominated in
this receipt. The three largest across-execution figures are 2.726 % at
`bit_backend/and_inplace/words=1`, 1.379 % at
`polynomial/div_rem_auto/dividend=4096/divisor=2047` and 1.330 % at
`bit_backend/xor_inplace/words=64`; the largest within-execution figure is
1.337 % at `bit_backend/and_inplace/words=1`. Both tripping cells sit far below
the band: `bit_backend/popcount/words=1` records 0.046--0.602 % within execution
and 0.492 % across, and `bit_backend/popcount/words=8` records 0.032--0.116 %
and 0.076 %.

## Control arm — enters no verdict

The control arm is the rebuild of the baseline revision that
[`across-build-control-arm-v1.md`](across-build-control-arm-v1.md) §2
predeclares, run in this session under the same wrapper, lock, affinity,
governor, toolchain, protocol and schema token as the post-cutover arm, with no
tuning profile installed. Everything in this section is recorded evidence for
the tracked rework. **None of it enters the verdict above.** Per control-arm §3
and §4.4 no ratio here changes a per-cell verdict, the geometric mean or the
`RESULT:` line, and the arm cannot convert a `FAIL` into a `PASS`, widen τ_cell
or τ_set, drop a cell, or close `50b47eae`.

### Which predeclared reading applies

The control build's bench binary hashes to
`c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, which
differs from the baseline's
`acbfd9c49325b4063b7a419114e05bacd6b98299bbde45fc89016f0d162db0fe`. Control-arm
§2 declares both readings in advance, and this is the second: **a different hash
means the arm carries an across-build component of the kind §1 records.** The
arm therefore bounds the across-build spread between two builds of
behaviourally unchanged selector code rather than the session-to-session
variation of one binary.

### Control-arm ratios against the baseline

Produced with the §5 comparison mode, whose verdict column and `RESULT:` line
carry no standing for this pair, the same way the baseline receipt records its
eligibility check without a verdict.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm.csv
```

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv candidate=dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.519421 3.730689 1.060029 FAIL
bit_backend/and_inplace/words=8 simd 4.611779 4.402874 0.954702 PASS
bit_backend/not_inplace/words=1 scalar 2.833877 2.833989 1.000039 PASS
bit_backend/not_inplace/words=8 simd 4.159037 4.202114 1.010357 PASS
bit_backend/or_inplace/words=1 scalar 4.157592 3.517992 0.846161 PASS
bit_backend/or_inplace/words=8 simd 4.651777 4.836143 1.039633 PASS
bit_backend/popcount/words=1 scalar 1.853240 1.802231 0.972476 PASS
bit_backend/popcount/words=8 simd 3.501248 3.510708 1.002702 PASS
bit_backend/xor_inplace/words=1 scalar 3.282865 2.856162 0.870021 PASS
bit_backend/xor_inplace/words=16 simd 4.237336 4.196850 0.990445 PASS
bit_backend/xor_inplace/words=4 scalar 3.945581 3.743325 0.948739 PASS
bit_backend/xor_inplace/words=64 simd 7.510168 7.368224 0.981100 PASS
bit_backend/xor_inplace/words=7 scalar 4.512804 4.621160 1.024011 PASS
bit_backend/xor_inplace/words=8 simd 3.785692 3.743840 0.988945 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14259449.788235 14272653.197647 1.000926 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57113481.640000 57266580.120000 1.002681 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 27445793.431818 26554166.022222 0.967513 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14261197.134118 14267591.343529 1.000448 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 56938298.770000 57147126.350000 1.003668 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 19245019.569231 19046864.701587 0.989704 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4312979.932867 3834672.513846 0.889100 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5290880.676068 5291775.969492 1.000169 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2301365.583704 2327112.957678 1.011188 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5091288.531148 5124887.906667 1.006599 PASS
polynomial/mul/len=16 schoolbook 316.335008 321.899677 1.017591 PASS
polynomial/mul/len=256 karatsuba 36942.861779 36489.716302 0.987734 PASS
polynomial/mul/len=32 schoolbook 1222.719206 1245.231413 1.018412 PASS
polynomial/mul/len=33 karatsuba 1140.814861 1144.645532 1.003358 PASS
polynomial/mul/len=64 karatsuba 3807.325330 3807.145825 0.999953 PASS
polynomial/mul_fast/len=128 ntt 11919.874440 12093.632711 1.014577 PASS
polynomial/mul_fast/len=32 mul_dispatch 1190.350051 1187.105858 0.997275 PASS
polynomial/mul_fast/len=512 ntt 53574.708467 54369.300727 1.014831 PASS
polynomial/mul_fast/len=64 mul_dispatch 3813.538569 3790.729612 0.994019 PASS
polynomial/mul_fast/len=65 ntt 11910.678494 12085.701359 1.014695 PASS
geometric_mean 0.987936 PASS
RESULT: FAIL```

A ratio below 1 is recorded and read the same as one above, per control-arm §2,
because §1's effect runs in both directions while §5's verdict is one-sided.
The geometric mean of the thirty-four control-arm ratios is 0.987936.

The arm reproduces part of the across-build effect the baseline's Falsification
record measures against the procedure-verification cohorts. It lands outside
±5 % at five cells — `bit_backend/or_inplace/words=1` at 0.846161,
`bit_backend/xor_inplace/words=1` at 0.870021,
`polynomial/div_rem_auto/dividend=4096/divisor=1024` at 0.889100,
`bit_backend/xor_inplace/words=4` at 0.948739 and
`bit_backend/and_inplace/words=1` at 1.060029 — and each of the five agrees with
cohort A's ratio for that cell to within 0.088 %, in the same direction. Four of
the nine cells that record names fall inside ±5 % here:
`bit_backend/not_inplace/words=1` at 1.000039 against cohort A's 0.848172,
`bit_backend/not_inplace/words=8` at 1.010357 against 0.949458,
`bit_backend/xor_inplace/words=16` at 0.990445 against 0.949387, and
`polynomial/batch_evaluate/coeffs=4096/points=4096` at 0.967513 against 0.946814.
A third build therefore carries its own per-cell split rather than a fixed
offset between revisions, which is what the baseline record's finding that the
effect tracks the binary predicts.

### Control-arm dispersion

| Cell | Within-execution CV | Across-execution CV |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | 0.028--0.183% | 0.058% |
| `bit_backend/and_inplace/words=8` | 0.064--0.165% | 0.120% |
| `bit_backend/not_inplace/words=1` | 0.022--0.100% | 0.071% |
| `bit_backend/not_inplace/words=8` | 0.024--0.242% | 2.273% |
| `bit_backend/or_inplace/words=1` | 0.015--0.057% | 0.107% |
| `bit_backend/or_inplace/words=8` | 0.028--0.168% | 0.122% |
| `bit_backend/popcount/words=1` | 0.083--0.234% | 4.358% |
| `bit_backend/popcount/words=8` | 0.039--0.669% | 0.829% |
| `bit_backend/xor_inplace/words=1` | 0.068--0.183% | 0.036% |
| `bit_backend/xor_inplace/words=16` | 0.050--0.168% | 0.221% |
| `bit_backend/xor_inplace/words=4` | 0.033--0.153% | 0.258% |
| `bit_backend/xor_inplace/words=64` | 0.046--0.266% | 1.022% |
| `bit_backend/xor_inplace/words=7` | 0.043--0.138% | 0.318% |
| `bit_backend/xor_inplace/words=8` | 0.036--0.167% | 0.156% |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.043--0.171% | 0.028% |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.053--0.175% | 0.146% |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.308--1.082% | 0.604% |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.041--0.167% | 0.043% |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.084--0.246% | 0.030% |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.130--0.686% | 0.714% |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.078--0.197% | 0.113% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.047--0.514% | 2.155% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.042--1.046% | 0.270% |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.044--0.141% | 0.194% |
| `polynomial/mul/len=16` | 0.137--0.308% | 0.104% |
| `polynomial/mul/len=256` | 0.042--0.109% | 0.114% |
| `polynomial/mul/len=32` | 0.121--0.217% | 0.099% |
| `polynomial/mul/len=33` | 0.076--0.147% | 0.083% |
| `polynomial/mul/len=64` | 0.106--2.108% | 0.412% |
| `polynomial/mul_fast/len=128` | 0.036--0.299% | 0.354% |
| `polynomial/mul_fast/len=32` | 0.088--0.167% | 0.086% |
| `polynomial/mul_fast/len=512` | 0.034--0.159% | 0.155% |
| `polynomial/mul_fast/len=64` | 0.042--0.157% | 0.231% |
| `polynomial/mul_fast/len=65` | 0.010--0.240% | 0.323% |

No cell reaches τ_cell on either statistic. The widest across-execution figure
is 4.358 % at `bit_backend/popcount/words=1`, and it comes from a single
execution: that cell's five execution-level pooled rates are 1.762559, 1.944990,
1.772534, 1.772619 and 1.769794 ns/call, so execution 2 stands about 10 % above
the other four, which agree to 0.6 %. Pooling absorbs it into the recorded
0.972476.

## Falsification record

### The profile read costs more than τ_cell at both popcount cells

The gated comparison fails at two of the thirty-four pinned cells:

| Cell | Arm | Baseline ns/call | Post-cutover ns/call | Ratio | Added cost |
|---|---|---:|---:|---:|---:|
| `bit_backend/popcount/words=1` | scalar | 1.853240 | 2.191107 | 1.182311 | +0.337867 ns |
| `bit_backend/popcount/words=8` | simd | 3.501248 | 3.726078 | 1.064214 | +0.224830 ns |

Both exceed τ_cell = 5 %, so the comparison is `RESULT: FAIL` and `50b47eae`
REQ-01 is unmet.

**What this contradicts.** `dev/active/220cab0b/design.md` §8 records the risk
that the `OnceLock` read adds an atomic load plus a branch to the cheapest
bit-logical operations, assesses the small-buffer case as thin, and mitigates it
by putting a small-buffer cell in the pinned set "so the cost is bounded by
measurement rather than by argument". The measurement now delivers that bound,
and at the cheapest pinned operation it is 18.2 %, which is 3.6 times the
tolerance the same epic predeclared for it. The mechanism is the one the design
describes — `popcount` at `crates/gf2-core/src/kernels/ops.rs:204` reaches
`select_backend_for_size` once per call at `:207`, and the profile read is at
`crates/gf2-core/src/kernels/backend.rs:105`, so the read happens once per bulk
operation rather than once per word. What the measurement contradicts is the
assessment of the cost, not the account of where it is paid.

**The excursion is not noise.** Within this receipt
`bit_backend/popcount/words=1` records 0.046--0.602 % within-execution and
0.492 % across-execution dispersion, and `bit_backend/popcount/words=8` records
0.032--0.116 % and 0.076 %. The five execution-level pooled rates of
`bit_backend/popcount/words=1` are 2.185806, 2.210310, 2.185573, 2.188057 and
2.185803 ns/call against the baseline's 1.853558, 1.855309, 1.849736, 1.855750
and 1.851857, so every execution of the post-cutover arm stands above every
execution of the baseline. §7's noise-dominated clause is triggered for no cell.

**What the control arm says.** Control-arm §4.3 fixes the reading before the
numbers exist: a tripping cell where the control arm sits near 1 is evidence
that the excursion at that cell is attributable to the cutover. The control arm
records 0.972476 at `bit_backend/popcount/words=1` and 1.002702 at
`bit_backend/popcount/words=8`, both inside ±5 % and the first of them in the
opposite direction to the excursion. That reading applies at both cells. It is a
quantitative input into the tracked rework and changes no verdict.

**The plan named these cells in advance.** §4 states that a `OnceLock` read is a
fixed cost of roughly a nanosecond against operations that cost a few
nanoseconds at one word, that `bit_backend/popcount/words=1` and its siblings
are where a 5 % band bites first, and that a tolerance those cells could not trip
would not bound the cost. The pinned set is doing what it was built to do; the
tolerance stays at its predeclared value and no cell is dropped.

### The across-build component masks the same cost at six further cells

Measured against the control build rather than against the baseline, six more
cells sit above 1.05 — five bit-backend and one polynomial:

| Cell | Post / control | Control / baseline | Post / baseline, the verdict |
|---|---:|---:|---:|
| `bit_backend/xor_inplace/words=1` | 1.076071 | 0.870021 | 0.936205 PASS |
| `bit_backend/or_inplace/words=1` | 1.063511 | 0.846161 | 0.899902 PASS |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 1.061688 | 0.889100 | 0.943948 PASS |
| `bit_backend/xor_inplace/words=8` | 1.058419 | 0.988945 | 1.046717 PASS |
| `bit_backend/xor_inplace/words=4` | 1.057371 | 0.948739 | 1.003169 PASS |
| `bit_backend/xor_inplace/words=16` | 1.052165 | 0.990445 | 1.042112 PASS |

Against the baseline all six pass, because the baseline binary is itself slower
at those cells than the control build is.

This is the effect plan §4's falsification record predicts for exactly this
comparison — "at those cells a cutover that adds no cost can trip the per-cell
rule, and one that adds a real cost can be masked" — observed in the run the
prediction was written for. It is recorded rather than accommodated.

The post-cutover-against-control pair is itself across two builds and carries
the §1 across-build component in full, so it is no more a verdict than the
control arm is; its whole-set geometric mean is 1.011050 against the verdict
pair's 0.998853. No predeclared rule reads it, and nothing here re-pins the set,
widens a tolerance or supersedes §5. It is recorded because the tracked rework
needs to know that the two failing cells are not the only cells where the
cutover's cost is visible against a same-session pre-cutover build.

## Tracked rework

Per control-arm §4.2 and `@/inv/falsification-preserved`, the rework this
excursion triggers is tracked rather than deferred. Issue `c42720ce`, "Reduce
the per-call cost of the bit-backend threshold read", owns it inside epic
`6dc81018`, and `50b47eae` depends on it.

**The resolution is to remove the cost.** Owner decision DEC-D of 2026-08-20
settles it that way rather than by widening τ_cell or τ_set or by amending the
epic's criterion, and that is why the `RESULT: FAIL` this receipt records does
not end the epic: the tolerance stands where it was predeclared, and the work
moves to the cost the measurement found.

**What `c42720ce` delivers, and what it does not.** It reduces the per-call cost
of resolving `bit_backend.simd_min_words` at the selection boundary while the
profile keeps governing that boundary at every size in the pinned set. It does
not settle whether the reduction suffices: that is decided by a fresh run of
this same unmodified procedure under `50b47eae`, against this same baseline and
the same predeclared tolerance.

**A compiled-in floor below which the profile is not consulted is not the
mechanism.** Such a floor removes the read only below itself, so fixing
`bit_backend/popcount/words=8` would need a floor above 8. The conservative
default is 8, and the committed host calibration of `5ecc9bf8`,
[`2026-08-20-host-calibration.md`](2026-08-20-host-calibration.md), selects 4
for this field. Any floor high enough to fix that cell would therefore put the
boundary beyond the profile's reach and make the calibration inert. The cost has
to come down while the profile still governs the boundary at every pinned size,
which is what constrains the fix.

## What stands unchanged

- **The tolerance.** τ_cell stays 5 % and τ_set stays 2 %, at their predeclared
  values. Neither is widened to admit this excursion.
- **The pinned set.** All thirty-four cells of plan §2 stand. No cell is added,
  dropped or re-bracketed, and both arms measure the same set.
- **The schema token.** `selector-non-regression-v1` is not bumped, so the
  baseline stays valid and comparable.
- **The baseline.** `2026-08-19-pre-cutover-baseline.md` stands as taken. It is
  not re-run, not superseded and not adjusted.
- **This run.** It stands as taken. Plan §7 and control-arm §4.5 forbid
  repeating a measurement until it agrees with the tolerance, so the session is
  run once and its result recorded, as the baseline stands against its own
  contradicting measurement.

# Second post-cutover receipt for the pinned selector non-regression set

This is the second post-cutover receipt of issue `50b47eae`: one execution of
the procedure of
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md) from
a clean revision, taken after issue `c42720ce` landed the reduction of the
per-call threshold cost, together with the across-build control arm that issue
`51058f8e` predeclares in
[`across-build-control-arm-v1.md`](across-build-control-arm-v1.md). Both arms
run in one lock session.

The procedure runs unmodified. The plan, the harness, the pinned cell set of §2,
the tolerance of §4 and the comparison rule of §5 stand as issue `e8fe47f5`
pins them, the comparison is against the same baseline
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md), and
this run changes nothing under `crates/`.

[`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md) is
the preserved record of the first session's excursion. It stands as taken: it is
not superseded, re-run or adjusted by this receipt, and its `RESULT: FAIL`
remains the record of what the cutover measured before `c42720ce`. `c42720ce`,
"Reduce the per-call cost of the bit-backend threshold read", is the work that
excursion tracked, and this session measures whether it brings the pinned
comparison inside the predeclared tolerance.

**It does not.** The plan's §5 rule applied to the baseline against this receipt
reports `RESULT: FAIL` on both rules: eleven of the thirty-four cells exceed
τ_cell = 5 %, and the geometric mean of the thirty-four ratios is 1.027550,
above the predeclared τ_set = 1.02. The comparison is further outside its
tolerance than the first session's, at the two cells that failed then and at
nine cells besides. The Falsification record below carries every tripping cell
with both pooled rates and its ratio, the two cells this receipt records as
noise-dominated under §7, and the contradictions the run raises. No predeclared
value moves, and per plan §7 and control-arm §4.5 this run is not repeated until
it agrees.

## Post-cutover state

`select_backend_for_size` at `crates/gf2-core/src/kernels/backend.rs:104` is
`#[inline]` and reads the published threshold through
`crate::tuning::active_simd_min_words()` at `:106`; the definition site of the
conservative default it resolves to is the compiled-in `SIMD_MIN_WORDS_DEFAULT`
at `:83`.

The threshold lives in one `AtomicUsize`, `ACTIVE_SIMD_MIN_WORDS` at
`crates/gf2-core/src/tuning/mod.rs:650`, beside the `AtomicBool` resolved flag
`ACTIVE_SIMD_MIN_WORDS_RESOLVED` at `:648`. The inlined reader
`active_simd_min_words` at `:656` loads the flag `Acquire` and, when it is set,
loads the threshold `Relaxed`; when it is unset it calls the `#[cold]`
`resolve_active_simd_min_words` at `:666`. Each publication site stores the
threshold `Relaxed` and then the flag `Release`: the cold path at `:666`,
`active` at `:675`, and `install` at `:692`.

The five pinned bit-logical entry points reach the selector through
`crates/gf2-core/src/kernels/ops.rs`: `xor_inplace` at `:93`, `and_inplace` at
`:117`, `or_inplace` at `:154`, `not_inplace` at `:182`, and `popcount` at
`:204`.

The four polynomial thresholds resolve through the unchanged `OnceLock` walk of
`tuning::active()`. No polynomial dispatch path is touched by `c42720ce`; the
whole diff in `crates/gf2-core/src/` between the first session's post-cutover
revision `e2e2ced78a1d544c9dd4f545a58d5bcf1e751be2` and this session's
`bba06631533e8316ef3a4e317b2ae2971f9825ad` is the one-line selector change at
`backend.rs:104`--`:106` and the threshold cache in `tuning/mod.rs`.

Behaviour is unchanged at every pinned size: `--self-check` resolves
`simd_min_words=8` in both checkouts and `--list-cells` reproduces the plan's §2
arms cell for cell, so the comparison's per-cell identity precondition holds and
no cell changes arm.

## Result

Each arm records 850 rows: thirty-four cells x five executions x five
repetitions. Every row of the post-cutover arm carries `source_dirty=false` and
revision `bba06631533e8316ef3a4e317b2ae2971f9825ad`; every row of the control
arm carries `source_dirty=false` and revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6`. Every cell's equivalence probe
passes in both arms — a failed probe aborts the run, and both arms exit 0 with
the full row count.

The plan's §5 comparison of the baseline against the post-cutover arm reports:

- **τ_cell fails at eleven cells.** Six bit-backend and five polynomial. The
  Falsification record tabulates all eleven with both pooled rates and their
  ratios; the widest is `bit_backend/popcount/words=1` at 1.294670.
- **τ_set fails.** The geometric mean of the thirty-four per-cell ratios is
  1.027550, above the predeclared 1.02.
- The rule requires both to hold, so the comparison is `RESULT: FAIL` and exits
  non-zero.

Two cells exceed τ_cell on their own across-execution dispersion inside this
receipt and are recorded as noise-dominated under plan §7:
`bit_backend/xor_inplace/words=7` at 22.254 % and
`bit_backend/xor_inplace/words=64` at 12.081 %. Both are also tripping cells.
The tolerance is not widened for them and neither cell is dropped. No other cell
in either arm reaches τ_cell on either dispersion statistic; the control arm's
widest across-execution figure is 2.864 %.

## Reproducible protocol and provenance

Every value below is observed during this run. The two arms share one host
state, one lock, one affinity mask and one wrapper invocation; the rows that
differ between them are marked.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | [`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md), executed unmodified; control arm per [`across-build-control-arm-v1.md`](across-build-control-arm-v1.md) §2 |
| Source revision, post-cutover arm | `bba06631533e8316ef3a4e317b2ae2971f9825ad`; every row records `source_dirty=false` |
| Source revision, control arm | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the baseline receipt records; every row records `source_dirty=false` |
| Bench binary SHA-256, post-cutover arm | `1f29c25161dd049af178a8f2737ea47e9381fcab228b7e103a54cc792d9a1c25` |
| Bench binary SHA-256, control arm | `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710` |
| Control-arm checkout | `.agents/worktrees/control-0c072d73`, detached at `0c072d73ca65cf50af98b8c4b61ed876f8218df6` and clean; its harness file hashes to `ca605cc35adb2047110010905a502d075c40839d69ad8b065d71c3b3667f7f84`, byte-identical to the post-cutover checkout's, and the four public polynomial thresholds hold the same values in both. Its `gf2-core` release artifacts are removed with `cargo +1.95.0 clean -p gf2-core --release` before the build, so the binary is compiled in this session rather than reused |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0 bench`, both arms |
| Features | `simd`, both arms |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`, one invocation holding `/tmp/gf2-ccx1.lock` for both arms; the lock is unheld at start and acquired without blocking, and `/proc/locks` inside the wrapper records `FLOCK ADVISORY WRITE` by the wrapper's own PID 2461894 on `00:2d:39210`, the lock file's device and inode as `stat` reports them (`45:39210`). CPUs 6--11, observed as `Cpus_allowed_list: 6-11` inside the wrapper |
| Niceness | The wrapper's best-effort `nice -n -5` is denied to this non-root user; `nice` reports `cannot set niceness: Permission denied` and the child runs at niceness 0. The lock and the affinity remain in force |
| Host quiescence | No `permanent-campaign-runner` process, matched against `ps -eo pid,comm,args`. The one-minute load average is read down from 0.30 after the two bench builds and observed at 0.14 before the session and 0.12 as the wrapper is invoked; the five- and fifteen-minute averages are 0.36 and 2.70, decaying from earlier CI work. At the end of the session the three figures read 1.12, 0.95 and 2.07 |
| Timed work | Five fresh executions of five repetitions per arm, `--target-ms 250` |
| Measured duration | 2026-08-20 15:16:53--15:24:21 UTC (7 min 28 s), the wrapper's whole child lifetime, covering both arms |
| Raw file, post-cutover arm | [`2026-08-20-post-cutover-receipt-2.csv`](2026-08-20-post-cutover-receipt-2.csv), SHA-256 `447dda463a51db8c7ac8f4037d1c21f98f4f2491aef94b56db995c529e09a532` |
| Raw file, control arm | [`2026-08-20-post-cutover-control-arm-2.csv`](2026-08-20-post-cutover-control-arm-2.csv), SHA-256 `c0412967797c6cdde58ebd8938b343659924c2956af4552e7ab80ea75f9fa7c0` |

Both bench binaries are built before the timed run, so each execution reports
`Finished bench profile in 0.04s` to `0.06s` and no compilation happens under
the lock. The raw files are written to
`/tmp/gf2-e8fe47f5-post-cutover-2-bba06631.csv` and
`/tmp/gf2-e8fe47f5-control-arm-2-0c072d73.csv`, both checked absent beforehand,
and copied byte-for-byte into this directory afterwards; `cmp` reports each copy
identical and each pair of paths hashes to the SHA-256 above.

## Arm ordering

The ten executions of the session interleave the two arms in the counterbalanced
order **C P P C C P P C C P**, where C is the control arm and P is the
post-cutover arm — the order the first session used:

| Slot | Arm | Execution | Start (UTC) |
|---:|---|---:|---|
| 1 | control | 1 | 15:16:53 |
| 2 | post-cutover | 1 | 15:17:38 |
| 3 | post-cutover | 2 | 15:18:23 |
| 4 | control | 2 | 15:19:08 |
| 5 | control | 3 | 15:19:52 |
| 6 | post-cutover | 3 | 15:20:37 |
| 7 | post-cutover | 4 | 15:21:22 |
| 8 | control | 4 | 15:22:06 |
| 9 | control | 5 | 15:22:51 |
| 10 | post-cutover | 5 | 15:23:36 |

Running each arm as one contiguous block would confound arm with session half,
so a warm-up or thermal trend over the session would land entirely on one arm.
Interleaving in ABBA blocks removes that confound: each arm holds two of the
first four slots and two of the last four. Five executions per arm cannot
balance the mean slot position exactly — the ten positions sum to 55, so an
equal split would need 27.5 each — and the residual is one slot, with the
control arm at mean position 5.4 and the post-cutover arm at 5.6. Repeating the
first session's assignment keeps the two sessions ordered alike and leaves the
post-cutover arm at the later mean position, which is the direction that
penalises it under a warming session rather than the direction that would favour
a pass.

The residual matters only to the extent the session drifts. Taking each
execution's set-level geometric mean against its own cell's five-execution mean,
the post-cutover arm reads 0.993966, 1.009194, 1.004365, 0.995919 and 0.993167
across executions 1--5, and the control arm reads 0.999579, 1.000794, 1.000738,
1.000182 and 0.998576. The control arm stays inside ±0.15 % with no monotone
trend. The post-cutover arm's spread is wider, ±0.9 % and non-monotone, and it
is driven by the single-execution outliers the Falsification record records at
`bit_backend/xor_inplace/words=7`, `bit_backend/xor_inplace/words=64` and
`polynomial/mul/len=33` rather than by a session trend; the control executions
interleaved between them are clean.

## Step 0 — harness against the plan

`--self-check` prints the same line in both checkouts:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

`per_cell_tolerance=0.050000` and `set_tolerance=0.020000` equal the plan's §4
values, and `cells=34` equals its §2 count. `simd_min_words=8` is the value the
post-cutover checkout resolves through the published threshold and the value the
control checkout reads from its compiled-in constant, so the two agree on the
guard the bit-backend cells straddle.

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
test ! -e /tmp/gf2-e8fe47f5-post-cutover-2-bba06631.csv
test ! -e /tmp/gf2-e8fe47f5-control-arm-2-0c072d73.csv
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash -l run-both-arms-2.sh
```

`run-both-arms-2.sh` is the session driver. It records the host facts this
receipt cites, then runs the ten executions:

```sh
#!/usr/bin/env bash
set -euo pipefail

MAIN=/home/vkaskivuo/Projects/gf2
CTRL_TREE=/home/vkaskivuo/Projects/gf2/.agents/worktrees/control-0c072d73
POST_OUT=/tmp/gf2-e8fe47f5-post-cutover-2-bba06631.csv
CTRL_OUT=/tmp/gf2-e8fe47f5-control-arm-2-0c072d73.csv

echo "child-start-utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "cpus-allowed: $(awk '/Cpus_allowed_list/{print $2}' /proc/self/status)"
echo "niceness: $(ps -o ni= -p $$ | tr -d ' ')"
echo "lock-inode: $(stat -c '%d:%i' /tmp/gf2-ccx1.lock)"
echo "proc-locks-flock:"
grep -i flock /proc/locks || true
echo "self-pid: $$"
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
| `bit_backend/and_inplace/words=1` | scalar | 4.168090 |
| `bit_backend/and_inplace/words=8` | simd | 4.630493 |
| `bit_backend/not_inplace/words=1` | scalar | 2.418077 |
| `bit_backend/not_inplace/words=8` | simd | 4.178487 |
| `bit_backend/or_inplace/words=1` | scalar | 4.003921 |
| `bit_backend/or_inplace/words=8` | simd | 5.270356 |
| `bit_backend/popcount/words=1` | scalar | 2.399335 |
| `bit_backend/popcount/words=8` | simd | 3.932501 |
| `bit_backend/xor_inplace/words=1` | scalar | 3.150567 |
| `bit_backend/xor_inplace/words=16` | simd | 4.425229 |
| `bit_backend/xor_inplace/words=4` | scalar | 3.949853 |
| `bit_backend/xor_inplace/words=64` | simd | 8.210747 |
| `bit_backend/xor_inplace/words=7` | scalar | 5.183035 |
| `bit_backend/xor_inplace/words=8` | simd | 3.965510 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | horner | 14,268,010.049412 |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | horner | 57,247,191.530000 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | subproduct | 26,458,267.764444 |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | horner | 14,266,695.364706 |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | horner | 57,110,087.450000 |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | subproduct | 18,707,900.913846 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | schoolbook | 3,834,971.916308 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | schoolbook | 5,286,925.058369 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | newton | 2,299,290.806296 |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | newton | 5,076,083.506939 |
| `polynomial/mul/len=16` | schoolbook | 317.965315 |
| `polynomial/mul/len=256` | karatsuba | 39,371.936392 |
| `polynomial/mul/len=32` | schoolbook | 1,222.676321 |
| `polynomial/mul/len=33` | karatsuba | 1,223.735809 |
| `polynomial/mul/len=64` | karatsuba | 4,105.692440 |
| `polynomial/mul_fast/len=128` | ntt | 11,828.901043 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,282.055056 |
| `polynomial/mul_fast/len=512` | ntt | 53,591.974278 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 4,094.866912 |
| `polynomial/mul_fast/len=65` | ntt | 11,840.599150 |

## Comparison against the baseline — the verdict

This is the comparison the plan's §5 defines and the epic gates on: the
pre-cutover baseline of `278acf3a` against this receipt, on pooled ns/call per
cell, judged by τ_cell = 5 % and τ_set = 2 %.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt-2.csv
```

The run prints all thirty-four per-cell lines and a geometric mean, so the §5
preconditions hold: both receipts carry schema `selector-non-regression-v1`,
their cell sets are identical and equal to the pinned set of §2, and every
cell's arm, family and operand sizes agree. The failure is therefore a tolerance
failure rather than a `RESULT: FAIL (selector identity mismatch)`.

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv candidate=dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt-2.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.519421 4.168090 1.184311 FAIL
bit_backend/and_inplace/words=8 simd 4.611779 4.630493 1.004058 PASS
bit_backend/not_inplace/words=1 scalar 2.833877 2.418077 0.853275 PASS
bit_backend/not_inplace/words=8 simd 4.159037 4.178487 1.004677 PASS
bit_backend/or_inplace/words=1 scalar 4.157592 4.003921 0.963039 PASS
bit_backend/or_inplace/words=8 simd 4.651777 5.270356 1.132977 FAIL
bit_backend/popcount/words=1 scalar 1.853240 2.399335 1.294670 FAIL
bit_backend/popcount/words=8 simd 3.501248 3.932501 1.123171 FAIL
bit_backend/xor_inplace/words=1 scalar 3.282865 3.150567 0.959701 PASS
bit_backend/xor_inplace/words=16 simd 4.237336 4.425229 1.044342 PASS
bit_backend/xor_inplace/words=4 scalar 3.945581 3.949853 1.001083 PASS
bit_backend/xor_inplace/words=64 simd 7.510168 8.210747 1.093284 FAIL
bit_backend/xor_inplace/words=7 scalar 4.512804 5.183035 1.148518 FAIL
bit_backend/xor_inplace/words=8 simd 3.785692 3.965510 1.047499 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14259449.788235 14268010.049412 1.000600 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57113481.640000 57247191.530000 1.002341 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 27445793.431818 26458267.764444 0.964019 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14261197.134118 14266695.364706 1.000386 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 56938298.770000 57110087.450000 1.003017 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 19245019.569231 18707900.913846 0.972091 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4312979.932867 3834971.916308 0.889170 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5290880.676068 5286925.058369 0.999252 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2301365.583704 2299290.806296 0.999098 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5091288.531148 5076083.506939 0.997014 PASS
polynomial/mul/len=16 schoolbook 316.335008 317.965315 1.005154 PASS
polynomial/mul/len=256 karatsuba 36942.861779 39371.936392 1.065752 FAIL
polynomial/mul/len=32 schoolbook 1222.719206 1222.676321 0.999965 PASS
polynomial/mul/len=33 karatsuba 1140.814861 1223.735809 1.072686 FAIL
polynomial/mul/len=64 karatsuba 3807.325330 4105.692440 1.078367 FAIL
polynomial/mul_fast/len=128 ntt 11919.874440 11828.901043 0.992368 PASS
polynomial/mul_fast/len=32 mul_dispatch 1190.350051 1282.055056 1.077040 FAIL
polynomial/mul_fast/len=512 ntt 53574.708467 53591.974278 1.000322 PASS
polynomial/mul_fast/len=64 mul_dispatch 3813.538569 4094.866912 1.073771 FAIL
polynomial/mul_fast/len=65 ntt 11910.678494 11840.599150 0.994116 PASS
geometric_mean 1.027550 FAIL
RESULT: FAIL
```

The comparison exits 1.

## The two cells the first session failed

The first receipt records `bit_backend/popcount/words=1` and
`bit_backend/popcount/words=8` as the two cells that exceeded τ_cell before
`c42720ce`. Both are still outside the tolerance, and both are further outside
it than they were:

| Cell | Arm | Baseline ns/call | First session ns/call | First ratio | This session ns/call | This ratio | Change in ratio |
|---|---|---:|---:|---:|---:|---:|---:|
| `bit_backend/popcount/words=1` | scalar | 1.853240 | 2.191107 | 1.182311 | 2.399335 | 1.294670 | +0.112359 |
| `bit_backend/popcount/words=8` | simd | 3.501248 | 3.726078 | 1.064214 | 3.932501 | 1.123171 | +0.058957 |

Against τ_cell = 1.05 the two cells stand 0.244670 and 0.073171 above the bar,
which is 23.30 % and 6.97 % beyond it. There is no headroom at either cell: the
reduction the excursion tracked does not appear in this comparison, and the
measured cost at both cells rises.

Neither cell is noise-dominated. `bit_backend/popcount/words=1` records
0.020--0.233 % within-execution and 0.143 % across-execution dispersion in this
arm, and its five execution-level pooled rates are 2.399219, 2.405295, 2.397175,
2.397130 and 2.397871 ns/call against the baseline's 1.853558, 1.855309,
1.849736, 1.855750 and 1.851857 — every post-cutover execution stands above
every baseline execution. `bit_backend/popcount/words=8` records 0.029--0.067 %
and 0.058 %, with execution rates 3.936583, 3.931485, 3.931801, 3.931393 and
3.931306 against the baseline's 3.496518, 3.499584, 3.501582, 3.504287 and
3.504286.

## Per-cell dispersion

Two statistics, as the procedure-verification receipt defines them.
Within-execution dispersion is the minimum--maximum sample coefficient of
variation across the five repetitions of an execution; across-execution
dispersion is the sample coefficient of variation of the five execution-level
pooled rates. The plan's §7 judges a cell's own dispersion against τ_cell = 5 %.

### Post-cutover arm

| Cell | Within-execution CV | Across-execution CV |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | 0.018--0.053% | 0.071% |
| `bit_backend/and_inplace/words=8` | 0.029--0.253% | 0.127% |
| `bit_backend/not_inplace/words=1` | 0.027--0.522% | 0.149% |
| `bit_backend/not_inplace/words=8` | 0.039--0.855% | 0.321% |
| `bit_backend/or_inplace/words=1` | 0.022--0.371% | 3.112% |
| `bit_backend/or_inplace/words=8` | 0.019--0.136% | 0.046% |
| `bit_backend/popcount/words=1` | 0.020--0.233% | 0.143% |
| `bit_backend/popcount/words=8` | 0.029--0.067% | 0.058% |
| `bit_backend/xor_inplace/words=1` | 0.040--0.226% | 0.099% |
| `bit_backend/xor_inplace/words=16` | 0.043--0.158% | 0.159% |
| `bit_backend/xor_inplace/words=4` | 0.045--0.058% | 0.025% |
| `bit_backend/xor_inplace/words=64` | 0.132--0.293% | 12.081% |
| `bit_backend/xor_inplace/words=7` | 0.021--0.244% | 22.254% |
| `bit_backend/xor_inplace/words=8` | 0.022--0.089% | 0.072% |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.033--0.088% | 0.031% |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.021--0.078% | 0.207% |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.091--0.223% | 0.924% |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.029--0.063% | 0.032% |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.025--0.102% | 0.021% |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.051--0.189% | 0.080% |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.028--0.066% | 0.022% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.041--0.660% | 2.504% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.010--0.075% | 0.042% |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.016--0.089% | 0.059% |
| `polynomial/mul/len=16` | 0.142--0.425% | 0.197% |
| `polynomial/mul/len=256` | 0.031--0.451% | 1.513% |
| `polynomial/mul/len=32` | 0.061--0.146% | 0.110% |
| `polynomial/mul/len=33` | 0.036--0.106% | 3.986% |
| `polynomial/mul/len=64` | 0.038--0.095% | 1.618% |
| `polynomial/mul_fast/len=128` | 0.024--0.053% | 0.028% |
| `polynomial/mul_fast/len=32` | 0.032--0.066% | 0.019% |
| `polynomial/mul_fast/len=512` | 0.026--0.055% | 1.426% |
| `polynomial/mul_fast/len=64` | 0.020--0.074% | 0.899% |
| `polynomial/mul_fast/len=65` | 0.024--0.434% | 0.142% |

Two cells exceed τ_cell on their across-execution statistic and are recorded as
noise-dominated under §7: `bit_backend/xor_inplace/words=7` at 22.254 % and
`bit_backend/xor_inplace/words=64` at 12.081 %. Both come from a single
execution. The five execution-level pooled rates of
`bit_backend/xor_inplace/words=7` are 4.820574, 7.489395, 4.824367, 4.825331 and
4.824860 ns/call, so execution 2 stands 55 % above the other four, which agree
to 0.1 %; those of `bit_backend/xor_inplace/words=64` are 7.798764, 7.875303,
10.098983, 7.876283 and 7.876445, so execution 3 stands 28 % above the other
four, which agree to 1.0 %. Within-execution dispersion at both cells stays
under 0.30 %, so the excursion is between processes rather than inside one. The
tolerance is not widened for either cell and neither is dropped.

Excluding those two, the widest across-execution figure in this arm is 3.986 %
at `polynomial/mul/len=33`, which is also a single-execution effect — its rates
are 1201.019145, 1202.701607, 1312.497642, 1203.526112 and 1206.235878, with
execution 3 about 9 % above the other four. The widest within-execution figure
in the arm is 0.855 % at `bit_backend/not_inplace/words=8`.

### Control arm

| Cell | Within-execution CV | Across-execution CV |
|---|---|---:|
| `bit_backend/and_inplace/words=1` | 0.036--0.045% | 0.034% |
| `bit_backend/and_inplace/words=8` | 0.032--0.225% | 0.140% |
| `bit_backend/not_inplace/words=1` | 0.028--0.053% | 0.063% |
| `bit_backend/not_inplace/words=8` | 0.024--0.078% | 2.254% |
| `bit_backend/or_inplace/words=1` | 0.022--0.078% | 0.095% |
| `bit_backend/or_inplace/words=8` | 0.012--0.065% | 0.038% |
| `bit_backend/popcount/words=1` | 0.057--0.366% | 0.087% |
| `bit_backend/popcount/words=8` | 0.036--0.178% | 0.037% |
| `bit_backend/xor_inplace/words=1` | 0.033--0.091% | 0.078% |
| `bit_backend/xor_inplace/words=16` | 0.026--0.083% | 0.108% |
| `bit_backend/xor_inplace/words=4` | 0.015--0.059% | 0.115% |
| `bit_backend/xor_inplace/words=64` | 0.063--0.290% | 2.864% |
| `bit_backend/xor_inplace/words=7` | 0.052--0.091% | 0.058% |
| `bit_backend/xor_inplace/words=8` | 0.026--0.107% | 0.108% |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.018--0.547% | 0.185% |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.037--0.316% | 0.065% |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.262--1.645% | 1.027% |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.033--0.058% | 0.050% |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.043--0.432% | 0.098% |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.365--0.988% | 0.712% |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.030--0.046% | 0.070% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.032--0.099% | 2.409% |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.039--0.115% | 0.279% |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.034--0.121% | 0.063% |
| `polynomial/mul/len=16` | 0.072--0.416% | 0.057% |
| `polynomial/mul/len=256` | 0.034--0.055% | 0.097% |
| `polynomial/mul/len=32` | 0.066--0.143% | 0.199% |
| `polynomial/mul/len=33` | 0.033--0.128% | 0.047% |
| `polynomial/mul/len=64` | 0.883--3.395% | 1.235% |
| `polynomial/mul_fast/len=128` | 0.019--0.069% | 0.126% |
| `polynomial/mul_fast/len=32` | 0.034--0.159% | 0.133% |
| `polynomial/mul_fast/len=512` | 0.026--0.047% | 0.253% |
| `polynomial/mul_fast/len=64` | 0.017--0.130% | 0.359% |
| `polynomial/mul_fast/len=65` | 0.021--0.063% | 0.138% |

No cell in the control arm reaches τ_cell on either statistic. Its widest
across-execution figure is 2.864 % at `bit_backend/xor_inplace/words=64` and its
widest within-execution figure is 3.395 % at `polynomial/mul/len=64`.

## Control arm — enters no verdict

The control arm is the rebuild of the baseline revision that
[`across-build-control-arm-v1.md`](across-build-control-arm-v1.md) §2
predeclares, run in this session under the same wrapper, lock, affinity,
governor, toolchain, protocol and schema token as the post-cutover arm, with no
tuning profile installed. Everything in this section is recorded evidence for
the disposition. **None of it enters the verdict above.** Per control-arm §3 and
§4.4 no ratio here changes a per-cell verdict, the geometric mean or the
`RESULT:` line, and the arm cannot convert a `FAIL` into a `PASS`, widen τ_cell
or τ_set, drop a cell, or close `50b47eae`.

### Which predeclared reading applies

The control build's bench binary hashes to
`c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, which
differs from the baseline's
`acbfd9c49325b4063b7a419114e05bacd6b98299bbde45fc89016f0d162db0fe`. Control-arm
§2 declares both readings in advance, and this is the second: **a different hash
means the arm carries an across-build component of the kind §1 records.** The
arm therefore bounds the across-build spread between two builds of behaviourally
unchanged selector code rather than the session-to-session variation of one
binary.

The rebuild is bit-identical to the first session's control build, which hashes
to the same value. The two control arms therefore run one binary in two
sessions, and the pair carries no across-build component at all; the
Falsification record uses it as a direct bound on session-to-session variation.

### Control-arm ratios against the baseline

Produced with the §5 comparison mode, whose verdict column and `RESULT:` line
carry no standing for this pair, the same way the baseline receipt records its
eligibility check without a verdict.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv candidate=dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.519421 3.729268 1.059625 FAIL
bit_backend/and_inplace/words=8 simd 4.611779 4.399479 0.953966 PASS
bit_backend/not_inplace/words=1 scalar 2.833877 2.832987 0.999686 PASS
bit_backend/not_inplace/words=8 simd 4.159037 4.198788 1.009558 PASS
bit_backend/or_inplace/words=1 scalar 4.157592 3.517819 0.846119 PASS
bit_backend/or_inplace/words=8 simd 4.651777 4.832222 1.038791 PASS
bit_backend/popcount/words=1 scalar 1.853240 1.768909 0.954495 PASS
bit_backend/popcount/words=8 simd 3.501248 3.499059 0.999375 PASS
bit_backend/xor_inplace/words=1 scalar 3.282865 2.853514 0.869215 PASS
bit_backend/xor_inplace/words=16 simd 4.237336 4.192990 0.989535 PASS
bit_backend/xor_inplace/words=4 scalar 3.945581 3.739582 0.947790 PASS
bit_backend/xor_inplace/words=64 simd 7.510168 7.458340 0.993099 PASS
bit_backend/xor_inplace/words=7 scalar 4.512804 4.614634 1.022565 PASS
bit_backend/xor_inplace/words=8 simd 3.785692 3.744107 0.989015 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14259449.788235 14280324.232941 1.001464 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57113481.640000 57322695.300000 1.003663 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 27445793.431818 26412612.973333 0.962356 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14261197.134118 14269938.705882 1.000613 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 56938298.770000 57135846.350000 1.003470 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 19245019.569231 18899609.098462 0.982052 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4312979.932867 3830311.953846 0.888089 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5290880.676068 5311647.957940 1.003925 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2301365.583704 2326171.744569 1.010779 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5091288.531148 5117894.173554 1.005226 PASS
polynomial/mul/len=16 schoolbook 316.335008 322.046195 1.018054 PASS
polynomial/mul/len=256 karatsuba 36942.861779 36459.005270 0.986903 PASS
polynomial/mul/len=32 schoolbook 1222.719206 1245.090343 1.018296 PASS
polynomial/mul/len=33 karatsuba 1140.814861 1143.608839 1.002449 PASS
polynomial/mul/len=64 karatsuba 3807.325330 3854.860397 1.012485 PASS
polynomial/mul_fast/len=128 ntt 11919.874440 12062.382450 1.011955 PASS
polynomial/mul_fast/len=32 mul_dispatch 1190.350051 1187.194135 0.997349 PASS
polynomial/mul_fast/len=512 ntt 53574.708467 54218.133498 1.012010 PASS
polynomial/mul_fast/len=64 mul_dispatch 3813.538569 3792.842479 0.994573 PASS
polynomial/mul_fast/len=65 ntt 11910.678494 12065.443754 1.012994 PASS
geometric_mean 0.987258 PASS
RESULT: FAIL
```

A ratio below 1 is recorded and read the same as one above, per control-arm §2,
because §1's effect runs in both directions while §5's verdict is one-sided. The
geometric mean of the thirty-four control-arm ratios is 0.987258.

The arm lands outside ±5 % at the same five cells as the first session's control
arm and in the same direction: `bit_backend/or_inplace/words=1` at 0.846119,
`bit_backend/xor_inplace/words=1` at 0.869215,
`polynomial/div_rem_auto/dividend=4096/divisor=1024` at 0.888089,
`bit_backend/xor_inplace/words=4` at 0.947790 and
`bit_backend/and_inplace/words=1` at 1.059625. That reproduces the across-build
effect the baseline's Falsification record measures, and it is the same effect
the first session's control arm recorded.

## Falsification record

### The comparison fails at eleven cells and on the set statistic

The gated comparison fails at eleven of the thirty-four pinned cells, ordered by
ratio:

| Cell | Arm | Baseline ns/call | Post-cutover ns/call | Ratio | Added cost |
|---|---|---:|---:|---:|---:|
| `bit_backend/popcount/words=1` | scalar | 1.853240 | 2.399335 | 1.294670 | +0.546095 ns |
| `bit_backend/and_inplace/words=1` | scalar | 3.519421 | 4.168090 | 1.184311 | +0.648669 ns |
| `bit_backend/xor_inplace/words=7` | scalar | 4.512804 | 5.183035 | 1.148518 | +0.670231 ns |
| `bit_backend/or_inplace/words=8` | simd | 4.651777 | 5.270356 | 1.132977 | +0.618579 ns |
| `bit_backend/popcount/words=8` | simd | 3.501248 | 3.932501 | 1.123171 | +0.431253 ns |
| `bit_backend/xor_inplace/words=64` | simd | 7.510168 | 8.210747 | 1.093284 | +0.700579 ns |
| `polynomial/mul/len=64` | karatsuba | 3,807.325330 | 4,105.692440 | 1.078367 | +298.367110 ns |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,190.350051 | 1,282.055056 | 1.077040 | +91.705005 ns |
| `polynomial/mul_fast/len=64` | mul_dispatch | 3,813.538569 | 4,094.866912 | 1.073771 | +281.328343 ns |
| `polynomial/mul/len=33` | karatsuba | 1,140.814861 | 1,223.735809 | 1.072686 | +82.920948 ns |
| `polynomial/mul/len=256` | karatsuba | 36,942.861779 | 39,371.936392 | 1.065752 | +2,429.074613 ns |

All eleven exceed τ_cell = 5 %. The geometric mean of the thirty-four ratios is
1.027550, which exceeds τ_set = 2 %, so the set rule fails as well. Both rules
must hold, so the comparison is `RESULT: FAIL` and `50b47eae` REQ-01 is unmet.

**What this contradicts.** The first receipt's Tracked rework section states
that `c42720ce` "reduces the per-call cost of resolving
`bit_backend.simd_min_words` at the selection boundary" and that whether the
reduction suffices "is decided by a fresh run of this same unmodified procedure
under `50b47eae`, against this same baseline and the same predeclared
tolerance". This is that run, and it decides against the reduction: at the two
cells the excursion named, the ratio rises from 1.182311 to 1.294670 and from
1.064214 to 1.123171, and nine further cells that passed in the first session
now trip the rule. Owner decision DEC-D settled the resolution as removing the
cost rather than widening the tolerance; the measurement now says the cost at
the pinned cells is not removed, and the comparison stands further outside the
tolerance than before.

**What the measurement does not isolate.** The post-cutover arm differs from the
first session's post-cutover arm by one binary, and that binary carries both the
threshold cache and whatever code and data layout the cache induces across the
crate. The whole diff in `crates/gf2-core/src/` between the two revisions is the
selector's `#[inline]` and its one-line read at `backend.rs:104`--`:106` and the
cache in `tuning/mod.rs`. This procedure compares binaries, so it bounds the
cost the pinned cells pay; it does not attribute that cost between the cache's
own instructions and the layout its introduction produces, and nothing in this
receipt claims such an attribution.

### Five polynomial cells move although no polynomial code changes

Five of the eleven tripping cells are polynomial: `polynomial/mul/len=33`,
`len=64` and `len=256`, and `polynomial/mul_fast/len=32` and `len=64`, between
6.6 % and 7.8 % above the baseline. Their dispatch path is untouched. The four
polynomial thresholds resolve through the unchanged `OnceLock` walk of
`tuning::active()`, `c42720ce` changes only the bit-backend threshold, and the
four public threshold constants hold identical values in both checkouts.

Measured against this session's control build rather than against the baseline,
the same five cells sit between 1.065069 and 1.079903, so the movement is
present against a same-session pre-cutover binary as well as against the
baseline. Measured against the first session's post-cutover build they sit
between 1.073951 and 1.080991.

The design's account of the cutover's cost — an atomic load plus a branch at the
selection boundary, paid once per bulk operation — does not reach these cells,
so the account does not explain what the pinned set now records. This is the
across-build component that plan §4's falsification record and control-arm §1
establish, observed at cells where it trips the rule rather than masks it. The
plan predicts exactly this for exactly this comparison: "at those cells a cutover
that adds no cost can trip the per-cell rule, and one that adds a real cost can
be masked." It is recorded rather than accommodated.

### The session-to-session bound: the same binary reproduces to 0.07 %

This session's control build is bit-identical to the first session's, both
hashing to
`c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`. Comparing
the two control arms therefore compares one binary across two sessions, with no
across-build component:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

That pair returns `RESULT: PASS` with a geometric mean of 0.999314 and every one
of the thirty-four cells inside ±1.9 %. The three widest deviations from 1 are
`bit_backend/popcount/words=1` at 0.981511, `polynomial/mul/len=64` at 1.012533
and `bit_backend/xor_inplace/words=64` at 1.012230; the remaining thirty-one sit
inside ±0.8 %. Session-to-session variation of a fixed binary on this host under
this wrapper is therefore bounded well inside τ_cell and an order of magnitude
inside τ_set.

The same pairing across the two post-cutover builds returns a geometric mean of
1.028730 with thirteen cells outside ±5 %, twelve of them above 1.05. The two
sessions differ in binary and in nothing else the measurement can see, so what
moved between them tracks the binary. This is a bound this pair establishes, not a verdict: it enters no
per-cell verdict, the geometric mean or the `RESULT:` line, and it re-pins,
widens and supersedes nothing.

### Two cells are noise-dominated in this receipt

Per plan §7, a cell whose own dispersion within a receipt exceeds τ_cell is
recorded as noise-dominated together with its numbers, the tolerance is not
widened for it, and the cell is not dropped. Two cells qualify, both in the
post-cutover arm and both also tripping cells:

| Cell | Across-execution CV | Execution-level pooled ns/call | Verdict ratio |
|---|---:|---|---:|
| `bit_backend/xor_inplace/words=7` | 22.254 % | 4.820574, 7.489395, 4.824367, 4.825331, 4.824860 | 1.148518 FAIL |
| `bit_backend/xor_inplace/words=64` | 12.081 % | 7.798764, 7.875303, 10.098983, 7.876283, 7.876445 | 1.093284 FAIL |

At both cells one execution stands far above four that agree closely, and
within-execution dispersion stays under 0.30 %, so the excursion is between
processes rather than inside one. Their verdicts stand as §5 computes them: the
pooled statistic is defined over all twenty-five windows of a cell, and §7
directs that such a cell is recorded, not dropped or accommodated.

Pooling only the four clean executions of each cell gives 1.068910 at
`bit_backend/xor_inplace/words=7` and 1.046122 at
`bit_backend/xor_inplace/words=64`. The first therefore trips τ_cell without its
outlying execution and the second does not. That arithmetic is a note on the
recorded numbers; it changes no verdict, and neither cell's recorded ratio is
restated by it.

The control arm shows no such execution at any cell, and no control cell reaches
τ_cell on either dispersion statistic.

## Disposition

The comparison fails, so `50b47eae` REQ-01 is unmet, and per control-arm §4.1
the verdict stands as §5 computes it. Per §4.5 and plan §7 this session is run
once and its result recorded; it is not repeated until it agrees, and no
predeclared value moves for it.

Two receipts now record a failing comparison under this procedure. The first,
[`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md),
stands as taken as the record of the excursion before `c42720ce`; this one
records what the same unmodified procedure measures after it. Control-arm §4.2
requires the rework an excursion triggers to be tracked rather than deferred.
Deciding what that rework is — and whether the eleven-cell, two-rule failure
recorded here is one question or two, given that five of the eleven cells lie
outside any path the cutover touches — belongs to the owner of epic `6dc81018`,
not to this receipt.

## What stands unchanged

- **The tolerance.** τ_cell stays 5 % and τ_set stays 2 %, at their predeclared
  values. Neither is widened to admit this excursion.
- **The pinned set.** All thirty-four cells of plan §2 stand. No cell is added,
  dropped or re-bracketed, and both arms measure the same set, including the two
  cells this receipt records as noise-dominated.
- **The schema token.** `selector-non-regression-v1` is not bumped, so the
  baseline stays valid and comparable.
- **The baseline.** `2026-08-19-pre-cutover-baseline.md` stands as taken. It is
  not re-run, not superseded and not adjusted, and it is the baseline both
  sessions compare against.
- **The first receipt.** `2026-08-20-post-cutover-receipt.md` stands as taken.
  This receipt does not modify, supersede or re-run it; its excursion and its
  contradiction remain preserved in the record, as REQ-02 requires.
- **This run.** It stands as taken.

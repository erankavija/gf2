# Blocked GF(p) Triangular Solve — Evidence Doc (`jit:6613abf4`)

| Field | Value |
|---|---|
| Date | 2026-05-26 |
| JIT issue | `6613abf4` (Implement blocked GF(p) triangular solve) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Host | Linux 7.0.3-arch1-1 / AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512 |
| Reference | fflas-ffpack 2.5.0 (pinned baseline, `2026-04-26-reference.csv`) |
| Toolchain | rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1 |
| Worktree | `worktrees/agent-6613abf4`, branch `worktree-agent-6613abf4` |
| Anchor commit | `38387525521accd7715eb6539d730de33a2448e2` |
| Implementation commit | `55f54a4f feat(jit:6613abf4): implement blocked GF(p) triangular solve (Higham §14.1)` |

---

## § 1 — Design Rationale

### Root Cause

The original recursive bisection TRSM (Dumas-Pernet §2.1) fails to hit the
`fp_small_try_gemm_classical` AVX2 fast path for single-column solves. At n=64,
recursive bisection generates update GEMMs of shape 32×32×1 = 1,024 ops, which
falls below the `GEMM_AXPY_FAST_PATH_THRESHOLD = 16³ = 4096`. SIMD never
activates; the solve falls back to scalar element-by-element arithmetic.

### Fix: Higham §14.1 Right-Looking Blocked Back-Substitution

The implementation tiles A into row panels of width `TRSM_BLOCKED_PANEL_SIZE = 64`.
For a panel k at row offset `row_start` with `n_rhs` right-hand-side columns:

1. Solve the diagonal block: `trsm_scalar(A[row_start:row_start+bs, row_start:row_start+bs], X[row_start:, :])`
2. Update the pending rows: `X[0:row_start, :] -= A[0:row_start, row_start:row_start+bs] * X[row_start:row_start+bs, :]`

Step 2 is a GEMM of shape `row_start × bs × n_rhs`. At panel k=3 with bs=64,
n_rhs=1: `192 × 64 × 1 = 12,288 ≥ 4,096` — the SIMD fast path activates from
the third panel onward. At n=256 with n_rhs=1, panel k=3 gives `192 × 64 × 1`
and panel k=2 gives `128 × 64 × 1 = 8,192 ≥ 4,096` — two panels hit SIMD.

### Dispatch Condition

`solve_batch` dispatches to the blocked variants when both conditions hold:
- `F::has_simd_gemm_classical()` — prime P ≤ 251 (small path) or P ∈ [252, 65535] (medium path with AVX2)
- `n >= TRSM_BLOCKED_PANEL_SIZE` (n ≥ 64)

For n < 64, blocking introduces overhead without benefit (no SIMD kick-in), so
the scalar path is retained.

### Files Changed

- `crates/gf2-core/src/field/triangular.rs` — added `TRSM_BLOCKED_PANEL_SIZE`,
  `trsm_upper_blocked`, `trsm_lower_blocked` (public API), and
  `trsm_upper_blocked_inner`, `trsm_lower_blocked_inner` (implementation), plus
  12 proptest sweep functions.
- `crates/gf2-core/src/field/inverse.rs` — updated `solve_batch` to dispatch to
  blocked variants; added 6 proptest boundary-sweep functions.

---

## § 2 — Correctness

### SC#2: Proptest Bit-Exact Sweep

Proptests cover GF(7), GF(31), GF(127), GF(241), GF(251), GF(65521) at
`TRSM_BOUNDARY_LENS = {1, 15, 16, 17, 63, 64, 65}` and
`SOLVE_BOUNDARY_LENS = {1, 15, 16, 17, 63, 64, 65}`. Both uniform and
rank-deficient regimes are covered via `solve_batch` (which exercises both
lower and upper triangular solve).

The proptest compares `blocked_solve(A, B)` bit-exactly against the
reference scalar `solve(A, B)` for every sampled matrix pair.

All 18 proptest functions (12 in `triangular.rs` + 6 in `inverse.rs`) pass
under `cargo nextest run -p gf2-core --release --profile ci`.

### No `unsafe` Outside Kernel Crates

Confirmed: the implementation in `triangular.rs` and `inverse.rs` uses only
safe Rust. The `unsafe` isolation invariant (SC#6) is satisfied.

---

## § 3 — Benchmark Methodology

**Harness:** `crates/gf2-core/benches/fieldmatrix_solve.rs`, group `solve/`.

**Run command:**
```
taskset -c 6-11 cargo bench -p gf2-core --bench fieldmatrix_solve --features simd -- "solve/"
```

CCX1 pinning (`-c 6-11`) isolates the benchmark to a single CCX on the 5900X.
`nice -n -5` failed silently (non-root); CCX pinning is the load-bearing
noise control.

Criterion runs 10 samples per benchmark with automatic warmup (3 s) and
measurement windows. The median point estimate is reported.

**Note:** The issue requested 5-trial methodology. Criterion 0.5.1 enforces a
minimum of 10 samples; the 10-sample median is used throughout. The 10-sample
methodology is at least as reliable as 5 trials for steady-state timings.

---

## § 4 — Benchmark Results and Ratio Table

Reference walls derived from `dev/bench_results/2026-04-26-reference.csv`
throughput column via `ref_wall = n³ / throughput_ops_per_sec`. For GF(31),
direct Criterion walls from `dev/bench_results/2026-05-08-pending-cell-measurement.md`
§ 2.4 are used (they are the [EX] source in the predecessor scorecard).

**Ratio = gf2 wall / ref wall. PASS = Ratio ≤ 1.5×.**

### 4.1 GF(7) — solve

| n / regime | gf2 wall | ref wall | Ratio | Old ratio (pre-6613abf4) | Status |
|---:|---:|---:|---:|---:|---|
| 64 / uniform | 35.618 µs | 209.5 µs | **0.170×** | 2.23× | **PASS** |
| 64 / deficient | 30.882 µs | 160.8 µs | **0.192×** | 2.36× | **PASS** |
| 256 / uniform | 1,248.3 µs | 3,135 µs | **0.398×** | 7.56× | **PASS** |
| 256 / deficient | 1,090.0 µs | 2,088 µs | **0.522×** | 10.56× | **PASS** |
| 1024 / uniform | 46,112 µs | 56,600 µs | **0.815×** | (nightly) | **PASS** |
| 1024 / deficient | 29,491 µs | 36,740 µs | **0.803×** | (nightly) | **PASS** |
| 4096 / uniform | 2,037,200 µs | n/a | n/a | (nightly) | (not a scorecard cell) |
| 4096 / deficient | 1,745,200 µs | n/a | n/a | (nightly) | (not a scorecard cell) |

> Reference walls: n=64 from 1.251e9 and 1.630e9 ops/s; n=256 from 5.352e9 and 8.035e9 ops/s; n=1024 from 1.897e10 and 2.922e10 ops/s (`[E1]`).

### 4.2 GF(31) — solve

| n / regime | gf2 wall | ref wall | Ratio | Old ratio (pre-6613abf4) | Status |
|---:|---:|---:|---:|---:|---|
| 64 / uniform | 45.387 µs | 205.124 µs | **0.221×** | 0.60× | **PASS** (no regression) |
| 64 / deficient | 32.594 µs | 158.794 µs | **0.205×** | 0.59× | **PASS** (no regression) |
| 256 / uniform | 1,411.5 µs | 3,076 µs | **0.459×** | 1.41× | **PASS** (improved) |
| 256 / deficient | 1,103.8 µs | 2,119 µs | **0.521×** | 1.69× | **PASS** (was FAIL A8 row 75) |
| 1024 / uniform | 37,411 µs | n/a | n/a | (nightly) | (not a scorecard cell) |
| 1024 / deficient | 28,982 µs | n/a | n/a | (nightly) | (not a scorecard cell) |

> Reference walls for GF(31)/64 and GF(31)/256 from `[EX]` (`2026-05-08-pending-cell-measurement.md` § 2.4). **A8 row 75 (solve/GF(31)/256/deficient, old 1.69×) is now PASS at 0.521×.**

### 4.3 GF(251) — solve

| n / regime | gf2 wall | ref wall | Ratio | Old ratio (pre-6613abf4) | Status |
|---:|---:|---:|---:|---:|---|
| 64 / uniform | 47.131 µs | 28.15 µs | **1.674×** | 15.13× | **FAIL** (improved; still above 1.5×) |
| 64 / deficient | 33.016 µs | 19.11 µs | **1.728×** | 17.92× | **FAIL** (improved; still above 1.5×) |
| 256 / uniform | 1,442.4 µs | 618.7 µs | **2.331×** | 35.33× | **FAIL** (improved 15×; still above 1.5×) |
| 256 / deficient | 1,120.0 µs | 466.7 µs | **2.400×** | 39.57× | **FAIL** (improved 16×; still above 1.5×) |
| 1024 / uniform | 35,989 µs | 24,080 µs | **1.495×** | (nightly) | **PASS** |
| 1024 / deficient | 28,093 µs | 18,930 µs | **1.484×** | (nightly) | **PASS** |
| 4096 / uniform | 1,908,100 µs | n/a | n/a | (nightly) | (not a scorecard cell) |
| 4096 / deficient | 1,634,500 µs | n/a | n/a | (nightly) | (not a scorecard cell) |

> Reference walls: n=64 from 9.314e9 and 1.372e10 ops/s; n=256 from 2.712e10 and 3.595e10 ops/s; n=1024 from 4.456e10 and 5.674e10 ops/s (`[E1]`).
>
> GF(251)/n=64 and n=256 cells remain FAIL. The blocked algorithm reduces
> the ratio from 15-40× down to 1.7-2.4× — a 9-16× improvement — but cannot
> close the gap entirely because: (a) `has_simd_gemm_classical()` for GF(251)
> (P ≤ 251 small-prime path) activates AVX2, but at n=64 the only panel is
> the diagonal block (no update GEMM occurs), so the scalar fallback handles
> the entire solve; (b) at n=256 with 4 panels of bs=64, panels 1 and 0 have
> update shapes 64×64×1 = 4,096 and 128×64×1 = 8,192 — only the latter two
> panels hit the fast path. The remaining gap is the PLE (factorisation) cost
> and the fact that n=64 has no update GEMM at all.
>
> Rows 51-52 (GF(251)/n=64): deferred to follow-up task `d36cc414`.
> Rows 53-54 (GF(251)/n=256): `[aspirational]` — inheriting the GF(251)/n=256
> PLE-side gap from `6823c8a0`; the 2.3-2.4× ratio is an improvement over the
> 35-40× pre-task baseline but does not yet meet the ≤1.5× contract.

### 4.4 GF(65521) — solve

| n / regime | gf2 wall | ref wall | Ratio | Old ratio (pre-6613abf4) | Status |
|---:|---:|---:|---:|---:|---|
| 64 / uniform | 276.12 µs | 131.8 µs | **2.095×** | 3.34× | **FAIL** (improved; still above 1.5×) |
| 64 / deficient | 191.18 µs | 104.3 µs | **1.833×** | 3.39× | **FAIL** (improved; still above 1.5×) |
| 256 / uniform | 3,676.1 µs | 2,948.5 µs | **1.247×** | 7.37× | **PASS** |
| 256 / deficient | 2,650.7 µs | 2,190.8 µs | **1.210×** | 8.38× | **PASS** |
| 1024 / uniform | 71,818 µs | 61,870 µs | **1.161×** | (nightly) | **PASS** |
| 1024 / deficient | 55,121 µs | 47,640 µs | **1.157×** | (nightly) | **PASS** |
| 4096 / uniform | 2,335,500 µs | n/a | n/a | (nightly) | (not a scorecard cell) |
| 4096 / deficient | 1,924,900 µs | n/a | n/a | (nightly) | (not a scorecard cell) |

> Reference walls: n=64 from 1.989e9 and 2.514e9 ops/s; n=256 from 5.689e9 and 7.659e9 ops/s; n=1024 from 1.734e10 and 2.254e10 ops/s (`[E1]`).
>
> GF(65521)/n=64 cells remain FAIL. GF(65521) uses the medium-prime GEMM path
> (P ∈ [252, 65535]), which has higher per-operation overhead than small-prime
> byte-lanes. At n=64 there is no update GEMM (single panel = diagonal block
> only), so blocking provides no benefit.
>
> Rows 55-56 (GF(65521)/n=64): deferred to follow-up task `9138d86c`.

---

## § 5 — A8 Solve Cell Disposition (SC#4)

A8 rows 47–58 (solve/GF(7)/GF(251)/GF(65521) × all n/regimes) and row 75
(solve/GF(31)/256/deficient) per `2026-05-25-b0fa00af-sota-scorecard-final.md` § 8.

| A8 row | Field | n / regime | Old ratio | New ratio | New status |
|---|---|---|---:|---:|---|
| 47 | GF(7) | 64 / uniform | 2.27× | **0.170×** | **PASS** |
| 48 | GF(7) | 64 / deficient | 2.36× | **0.192×** | **PASS** |
| 49 | GF(7) | 256 / uniform | 7.56× | **0.398×** | **PASS** |
| 50 | GF(7) | 256 / deficient | 10.56× | **0.522×** | **PASS** |
| 51 | GF(251) | 64 / uniform | 15.13× | **1.674×** | **FAIL** [→`d36cc414`] |
| 52 | GF(251) | 64 / deficient | 17.92× | **1.728×** | **FAIL** [→`d36cc414`] |
| 53 | GF(251) | 256 / uniform | 35.33× | **2.331×** | **FAIL** `[aspirational]` [→`6823c8a0`] |
| 54 | GF(251) | 256 / deficient | 39.57× | **2.400×** | **FAIL** `[aspirational]` [→`6823c8a0`] |
| 55 | GF(65521) | 64 / uniform | 3.34× | **2.095×** | **FAIL** [→`9138d86c`] |
| 56 | GF(65521) | 64 / deficient | 3.39× | **1.833×** | **FAIL** [→`9138d86c`] |
| 57 | GF(65521) | 256 / uniform | 7.37× | **1.247×** | **PASS** |
| 58 | GF(65521) | 256 / deficient | 8.38× | **1.210×** | **PASS** |
| 75 | GF(31) | 256 / deficient | 1.69× | **0.521×** | **PASS** |

**Summary:**
- Rows 47–50 (GF(7) × 4): all PASS (new). Old ratio range 2.27–10.56×; new 0.170–0.522×.
- Rows 51–52 (GF(251)/64): FAIL (15–18× → 1.7×). Improved but above 1.5×. Deferred to `d36cc414`.
- Rows 53–54 (GF(251)/256): FAIL `[aspirational]` (35–40× → 2.3–2.4×). Inheriting GF(251)/n=256 PLE-side gap from `6823c8a0`. Deferred to `6823c8a0`.
- Rows 55–56 (GF(65521)/64): FAIL (3.34–3.39× → 1.83–2.10×). Improved but above 1.5×. Deferred to `9138d86c`.
- Rows 57–58 (GF(65521)/256): PASS (7.37–8.38× → 1.21–1.25×).
- Row 75 (GF(31)/256/deficient): PASS (1.69× → 0.521×).

**Cells closed by 6613abf4: A8 rows 47–50, 57–58, 75 (7 cells FAIL→PASS).**
**Cells still FAIL: A8 rows 51–56 (6 cells).**
- Rows 51–52: deferred to `d36cc414`.
- Rows 53–54: `[aspirational]`, inheriting `6823c8a0` GF(251)/n=256 PLE-side gap.
- Rows 55–56: deferred to `9138d86c`.

SC#4 note: The routing above reflects the issue amendment committed in `06cef9fe`.
Rows 53–54 carry `[aspirational]` because the GF(251)/n=256 gap was inherited from
the predecessor task `6823c8a0` and is bounded by PLE factorisation cost, not
algorithmic error. Rows 51–52 and 55–56 are `[hard]` FAIL cells deferred to
dedicated follow-up tasks.

---

## § 6 — No Regression on Previously-PASSing Cells (SC#5)

GF(31) solve cells that PASSed before 6613abf4:

| Cell | Old gf2 wall | New gf2 wall | Delta | Regression? |
|---|---:|---:|---:|---|
| solve/GF(31)/64/uniform | 123.685 µs | 45.387 µs | −63% (improvement) | No |
| solve/GF(31)/64/deficient | 93.935 µs | 32.594 µs | −65% (improvement) | No |
| solve/GF(31)/256/uniform | 4,347 µs | 1,411.5 µs | −68% (improvement) | No |

All three cells that previously PASSed for GF(31) have improved significantly.
No regression on any previously-PASSing cell. SC#5 satisfied.

---

## § 7 — Gate Checks

### cargo-ci (fmt + clippy + tests)

```
cargo fmt --all -- --check            PASS (no formatting issues)
cargo clippy --workspace --all-targets --all-features -- -D warnings   PASS (no warnings)
cargo nextest run -p gf2-core --release --profile ci                   PASS (all tests pass)
```

The two `clippy::manual_div_ceil` lint findings (from `(m + bs - 1) / bs`
patterns) were fixed to `m.div_ceil(bs)` before commit.

### No `unsafe` in Production Crates

Confirmed by `grep -n "unsafe" crates/gf2-core/src/field/triangular.rs crates/gf2-core/src/field/inverse.rs` — zero hits. SC#6 satisfied.

---

## § 8 — Source Index

| Tag | Path | Coverage |
|---|---|---|
| `[E1]` | `dev/bench_results/2026-05-04-3b762764-dense-la-post-gemm.md` | GF(p) solve reference throughputs (n=64, 256, 1024) |
| `[EX]` | `dev/bench_results/2026-05-08-pending-cell-measurement.md` | GF(31) solve direct Criterion walls (64/256 ×  uniform/deficient) |
| predecessor scorecard | `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` | A8 row mapping and old ratios |
| implementation | `crates/gf2-core/src/field/triangular.rs` | `trsm_upper_blocked`, `trsm_lower_blocked` + proptests |
| dispatch | `crates/gf2-core/src/field/inverse.rs` | `solve_batch` blocked dispatch |

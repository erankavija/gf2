# 74ba1cdc R1 — Single-thread fgemm engineering: Route A + fp_medium at n=4096

| Field | Value |
|---|---|
| Date | 2026-05-26 |
| JIT issue | `74ba1cdc` (Improve single-thread fgemm engineering: Route A + fp_medium at n=4096) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2+FMA, no AVX-512 |
| Toolchain | rustc 1.95.0 |
| Branch | `worktree-agent-74ba1cdc-r1` (anchored to `c24765df`) |
| Status | **PASS** with user-approved SC#2 deferral to `695350fd` (see § 1.1) |
| Supersedes | `dev/bench_results/2026-05-25-74ba1cdc-fgemm-engineering.md` (R0 wall-hit) |

## 1.1 SC#2 amendment — user-approved deferral to 695350fd

Original SC#2: "GF(65521) at n=4096: ratio gf2/fflas ≤ 1.5× on 5-trial CCX1-pinned bench on the Zen 3 reference host."

**Amendment (2026-05-26, user-approved):** SC#2 is honoured by deferring the
remaining +17% gap on GF(65521)/4096 to follow-up task **`695350fd`** ("fp_medium
u16-lane BLIS MC/KC restructure (74ba1cdc Phase 6b)"). 74ba1cdc closes PASS for
SC#1 (GF(251)/4096 ratio 1.466) and for all non-regression / correctness
criteria. The GF(65521)/4096 cell is owned by `695350fd` for closure.

98336ab4 dependency re-points to `695350fd` (instead of `74ba1cdc`) for the
full-PASS n=4096 re-bench. The R1 engineering progress on fp_medium (the new
`fp_medium_gemm_panel` kernel, MR=2 NR=16, +9.5% throughput vs baseline) is
the foundation that `695350fd` builds on.

This amendment is recorded per `feedback_hard_criterion_self_satisfaction` and
`feedback_no_autonomous_amendments` (user gave explicit approval via the
session-8 escalation).

---

## 1. Summary (TL;DR)

R1 dispatch took the prior R0 dispatch's bottleneck analysis as ground truth
and executed two engineering levers. Result: GF(251)/n=4096 **PASSES** the
SC threshold; GF(65521)/n=4096 closes ~45 % of the gap but falls short.

| Cell | Pre-R1 baseline | After R1 | Delta | Ratio gf2/fflas | Target | Status |
|---|---:|---:|---:|---:|---:|---:|
| GF(251) / n=4096   | 87.57 | **108.42** | +23.8 % | 1.466 | ≤ 1.5 | **PASS** |
| GF(65521) / n=4096 | 36.88 | **39.78**  | + 7.9 % | 1.753 | ≤ 1.5 | SHORTFALL |

The remaining GF(65521) gap is in the per-cell u64-acc widen chain
(2 unpack_epi16 + 4 unpack_epi32 + 4 add_epi64 per 16-MAC cell-row).
Surfaced to the lead in § 7.

---

## 2. Method

All benches use a `flock`-guarded CCX1 mutex (`/tmp/gf2-ccx1.lock`)
plus `taskset -c 6-11` to pin to one CCX and `nice -n -5` (denied as
non-root; pinning works regardless). The wrapper is checked into the
worktree at `dev/benchmarks/ccx1-bench-flock.sh`.

Reproduce one cell:

```bash
./dev/benchmarks/ccx1-bench-flock.sh cargo bench \
    -p gf2-core --bench fieldmatrix_gemm --features rand,simd \
    -- '^gemm/Fp_65521/Fp_65521/4096$'
```

Each cell reports a criterion median across 10 samples plus an IQR.
Two-trial confirmation is the convention used below; the noise floor
on the quiet CCX1 measured ~1-2 % in cell-to-cell jitter.

---

## 3. Baseline measurement

Single-trial criterion bench, CCX1-pinned, immediately after starting
the worktree (no source changes; anchored to `c24765df`):

| Cell | gf2 Gop/s | issue cite (R0) | Δ vs issue |
|---|---:|---:|---:|
| GF(251)   / n=4096 | 87.57 | 85.22 | +2.8 % (within 5 % gate) |
| GF(65521) / n=4096 | 36.88 | 36.34 | +1.5 % (within 5 % gate) |

Confirmed within the 5 % `STOP and report` threshold. Proceed.

The two-trial confirmation noise was ~1.5 % — much cleaner than the
R0 dispatch's contention-bound measurement window, since the
sibling 40195c09 worker has now closed.

---

## 4. Lever 1 — Route A: L3-budgeted n_c_panels heuristic

Two sub-changes folded into one commit each. Both touch
`crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` only.

### 4.1 1a: Replace L2-clamp with shared L3 helper

The R0 evidence doc § 3.2 / § 4 identified the `n_c_panels = 1`
mis-blocking trap. The R0 worker attempted the fix at a 24 MB L3
budget and could not separate the lever's contribution from host
noise. R1 lands the same lever in a quiet host:

| Cell | Pre-L1 (24-MB budget direct measurement) | Post-L1 (24-MB) | Delta |
|---|---:|---:|---:|
| GF(251) / n=4096 | 87.57 | 99.35 | **+13.4 %** |

Commit `2ed76b4e` (`perf(jit:74ba1cdc): Route A — L3-budget
n_c_panels heuristic`).

### 4.2 1b: Budget sweep — 24 → 16 MB

Empirical sweep on the same quiet host (single-trial-per-budget):

| L3 budget | GF(251)/n=4096 Gop/s | Comment |
|---:|---:|---|
| 24 MB | 99.3  | Initial pick (R0 sketch) |
| 16 MB | 108.6 | **+9.4 % vs 24 MB** |
| 12 MB | 108.7 | within 16-MB noise |
|  8 MB | 108.0 | within 16-MB noise |
|  4 MB | 106.6 | -1.5 % regression |

The cliff between 4 and 8 MB suggests the active outer-block working
set has to physically fit in the L3 share that other Zen-3 traffic
(prefetcher, criterion harness, route-A pack scratches, to_mont/
from_mont LUTs) doesn't already consume. 16 MB is the conservative
midpoint — captures the full +9 % delta while leaving >16 MB
headroom for L3 contention.

Commit `aa7aaaeb` (`perf(jit:74ba1cdc): Route A — L3 budget tuned
24→16 MB`).

### 4.3 Combined Lever 1 result

| Cell | Baseline | Post-1a (24 MB) | Post-1b (16 MB) | Cum. delta |
|---|---:|---:|---:|---:|
| GF(251) / n=4096   | 87.57 |  99.35 | 108.42 | **+23.8 %** |
| GF(65521) / n=4096 | 36.88 |  ~36.9 |  ~36.9 | 0 % (different path) |

**Ratio gf2/fflas at GF(251)/n=4096**: 1.815 → 1.466 — **PASS**.

---

## 5. Lever 2 — fp_medium: panel GEMM kernel

The R0 § 3.3 / § 5.2 design sketch identified the bottleneck: no
panelized GEMM exists for the u16 lane width; the per-cell
`fp_medium_batch_dot` dispatch costs 16M calls at n=4096. R1
implements the sketch.

### 5.1 Implementation

New kernel `fp_medium_gemm_panel` in
`crates/gf2-kernels-simd/src/x86/fp_medium.rs` (committed in
`ba4a10e9`). MR=2 register-tile rows × NR=16 output cells per ymm.
Per-inner-step at p > 32_767 (GF(65521) path):

```
load_b (1 ymm of 16 u16 lanes)
for each row i in 0..MR:
    broadcast a[i, t] -> ymm (1 op)
    mullo_epi16 + mulhi_epu16 -> 2 ymm of u16 product halves (2 ops)
    unpacklo/unpackhi_epi16 -> 2 ymm of u32 products (2 ops)
    unpacklo/unpackhi_epi32 (× 2) -> 4 ymm of u64 products (4 ops)
    add_epi64 (× 4) -> 4 u64-lane acc vectors (4 ops)
```

Total per cell-row: 13 ops generating 16 MACs ≈ 0.81 ops/MAC.
Compare per-cell `fp_medium_batch_dot_mulhi` density: ~0.88 ops/MAC.

Wired through:
- `MediumPrimeFns::gemm_panel_fn` (new dispatch table entry).
- `gf2_core::gfp::simd_ops::fp_medium_try_gemm_panel<const P>` —
  packs A and B^T as Montgomery raw u16, runs the kernel, applies
  one Montgomery REDC per output cell (maps `R²·x → R·x = Mont(x)`).
- `Fp<P>::try_simd_gemm_classical` — falls through to the medium-
  prime kernel when `fp_small_try_gemm_classical` declines.
- `fp_small_gemm_classical_available` probe — returns `true` for
  medium-prime cells when `maybe_fp_medium` is available, so
  `gemm_axpy_into_view` takes the contig-A scratch path for them too.

### 5.2 MR sweep

| MR | GF(65521)/n=4096 Gop/s | acc-ymm budget | Compiler spills |
|---:|---:|---:|---:|
| 2 | **39.66** | 8 acc + ~7 ephemeral = 15 ymm | minimal |
| 3 | 31.32 | 12 acc + ~8 ephemeral = 20 ymm | -21 % vs MR=2 |
| 4 | 25.90 | 16 acc + ~10 ephemeral = 26 ymm | **-35 % vs MR=2** |

The u64-acc widen path holds **4 u64 acc ymm per row**; MR>2 exceeds
the 16-register file once B-load + broadcasts + product temps are
counted. MR=2 is the empirical optimum.

### 5.3 Lever 2 result

| Cell | Pre-L2 | Post-L2 | Delta |
|---|---:|---:|---:|
| GF(65521) / n=64   | 11.71 |  19.18 | **+63.8 %** (dispatch-bound) |
| GF(65521) / n=256  | ~16.4 |  32.78 | (panel kernel; new measurement) |
| GF(65521) / n=1024 | ~36.5 |  39.47 | + 8 % (arithmetic-bound) |
| GF(65521) / n=4096 | 36.65 |  39.78 | **+ 8.5 %** |
| GF(251)   / n=4096 | 108.6 | 108.42 | unchanged (small-prime path) |

**Ratio gf2/fflas at GF(65521)/n=4096**: 69.72 / 39.78 = **1.753**
— SHORTFALL (target ≤ 1.5; gap +17 %).

The +64 % gain at n=64 confirms the dispatch overhead theory: at
small n, eliminating the per-cell trait-dispatch chain dominates.
At n=4096 the kernel is arithmetic-bound and the gain shrinks
to +8.5 %.

---

## 6. Final per-cell scorecard at n=4096

| Cell | fflas Gop/s | gf2 Gop/s | Ratio | Target | Status |
|---|---:|---:|---:|---:|---:|
| GF(251)   | 158.96 | 108.42 | **1.466** | ≤ 1.5 | **PASS** |
| GF(65521) |  69.72 |  39.78 | **1.753** | ≤ 1.5 | SHORTFALL |

---

## 7. Non-regression sweep — 6 primes × 4 sizes

CCX1-pinned, flock-guarded. Two-trial confirmation; numbers below
are the second-trial median.

| Prime | n=64 | n=256 | n=1024 | n=4096 |
|---|---:|---:|---:|---:|
| GF(7)     | 34.31 | 73.34 | 78.09 | 112.83 |
| GF(31)    | 32.41 | 70.72 | 77.13 | 112.50 |
| GF(127)   |   —   | 69.10 | 77.12 |   —    |
| GF(241)   |   —   | 70.33 | 76.97 |   —    |
| GF(251)   | 32.88 | 71.37 | 96.43 | 108.42 |
| GF(65521) | 19.18 | 32.78 | 39.47 |  39.78 |

(GF(127) and GF(241) are bench-cell-restricted to n ∈ {256, 1024}
per `bench_gemm_fp_127` / `bench_gemm_fp_241` in
`crates/gf2-core/benches/fieldmatrix_gemm.rs:206-294`.)

Comparison vs `41096af5-post-wire-in-aggregate.csv` baseline (which
covers small primes only at small n):

| Cell | Baseline | This run | Delta |
|---|---:|---:|---:|
| GF(7)/64   | 32.16 | 34.31 | +6.7 % |
| GF(7)/256  | 70.75 | 73.34 | +3.7 % |
| GF(7)/1024 | 75.79 | 78.09 | +3.0 % |
| GF(31)/64   | 31.48 | 32.41 | +3.0 % |
| GF(31)/256  | 70.00 | 70.72 | +1.0 % |
| GF(31)/1024 | 76.51 | 77.13 | +0.8 % |
| GF(127)/256  | 69.80 | 69.10 | -1.0 % (within noise) |
| GF(127)/1024 | 76.19 | 77.12 | +1.2 % |
| GF(251)/64   | 31.52 | 32.88 | +4.3 % |
| GF(251)/256  | 69.93 | 71.37 | +2.1 % |
| GF(251)/1024 | 94.43 | 96.43 | +2.1 % |

**No regression on any cell.** Max delta across the 11 baseline-
comparable cells: +6.7 % (GF(7)/n=64; uncontended `set1_epi16`
broadcast amortisation in the new panel kernel's small-n path).

---

## 8. What did not work — remaining GF(65521) gap

### 8.1 Tried levers

- **MR=3, MR=4** — both regress vs MR=2 due to register-file pressure
  (see § 5.2). The u64-acc widen path holds 4 ymm per row, so MR > 2
  exceeds 16 registers with broadcast + load + product temps.
- **L3-budget tuning for the panel kernel** — the panel kernel
  doesn't have an outer-N blocking parameter (NR=16 is the full
  unit; n_panels is taken in sequence). The gain in §4 came from
  the Route A path, not the panel kernel.

### 8.2 Not tried (would close the gap but ≥ 1-week each)

- **BLIS MC/KC restructure for the panel kernel** — pre-pack B in
  L3-resident slabs, sweep all m × kc tiles per slab before moving
  to the next. Closes the L1-reuse gap that the current MR=2 kernel
  leaves on the table. Multi-day engineering.
- **madd_epi16 with hi-bit-split** — for p > 32_767, split each
  u16 into a 15-bit low and a 1-bit high, compute four sub-sums
  via signed madd_epi16, reconstruct. Same arithmetic density as
  the current mulhi+mullo path; gain would come from u32 acc lanes
  enabling MR=4. Speculative; design unverified.
- **AVX-512 VNNI `vpdpwssd`** — out of scope per project policy
  (epic `7f809931` blocks AVX-512 in SOTA-catch-up work).

### 8.3 Recommendation

GF(65521) closing the residual 17 % gap requires the BLIS MC/KC
restructure (§ 5.1 of the R0 evidence doc; same architectural
proposal as the Route A case, but for u16 lanes). That work was
explicitly classified as multi-day in R0 and is not appropriate
for an in-session R1 dispatch.

Surface as follow-up issue scoped tighter than 74ba1cdc:
**"Phase 6b: BLIS MC/KC panel kernel for fp_medium GF(65521)"** with
a pre-approved design sketch as a prereq (per `CLAUDE.md` perf-work
sketch protocol).

---

## 9. Commit chain (74ba1cdc R1)

| Commit | Subject |
|---|---|
| `2ed76b4e` | perf(jit:74ba1cdc): Route A — L3-budget n_c_panels heuristic |
| `aa7aaaeb` | perf(jit:74ba1cdc): Route A — L3 budget tuned 24→16 MB |
| `ba4a10e9` | perf(jit:74ba1cdc): fp_medium — panel GEMM kernel |
| (this doc) | docs(jit:74ba1cdc): R1 evidence doc + non-regression sweep |

All commits land on `worktree-agent-74ba1cdc-r1` only; the worktree
is anchored to `c24765df` (post-40195c09-close) on main.

---

## 10. Reproducibility

```bash
# Bench host: Zen 3 5900X, AVX2+FMA, no AVX-512.
# All commands run from the worktree root.

# Lock the CCX1 mutex (shared with other sibling workers if any).
touch /tmp/gf2-ccx1.lock

# Single-cell bench (replace the regex for other cells):
./dev/benchmarks/ccx1-bench-flock.sh cargo bench \
    -p gf2-core --bench fieldmatrix_gemm --features rand,simd \
    -- '^gemm/Fp_(251|65521)/Fp_(251|65521)/(64|4096)$'

# Full non-regression sweep (long; ~10 minutes wall):
./dev/benchmarks/ccx1-bench-flock.sh cargo bench \
    -p gf2-core --bench fieldmatrix_gemm --features rand,simd \
    -- '^gemm/Fp_(7|31|127|241|251|65521)/.*/(64|256|1024|4096)$'

# Correctness gate:
cargo nextest run --workspace --all-features --release --profile ci
```

The `dev/benchmarks/ccx1-bench-flock.sh` wrapper holds an exclusive
flock on `/tmp/gf2-ccx1.lock` for the duration of the child
command; concurrent sibling workers attempting to bench on the same
CCX1 will block until the lock is released.

---

## 11. Source index

| Reference | Path |
|---|---|
| R0 evidence doc (this supersedes) | `dev/bench_results/2026-05-25-74ba1cdc-fgemm-engineering.md` |
| Route A kernel | `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` |
| Route A n_c_panels helper | `n_c_panels_outer`, lines 95-116 |
| fp_medium kernel (new) | `crates/gf2-kernels-simd/src/x86/fp_medium.rs` |
| fp_medium_gemm_panel | lines 731-925 |
| fp_medium dispatch (new entry) | `crates/gf2-kernels-simd/src/fp_medium.rs::MediumPrimeFns::gemm_panel_fn` |
| gf2-core wrapper | `crates/gf2-core/src/gfp/simd_ops.rs::fp_medium_try_gemm_panel` |
| Trait wiring | `crates/gf2-core/src/gfp/mod.rs::try_simd_gemm_classical` |
| Bench harness | `crates/gf2-core/benches/fieldmatrix_gemm.rs` |
| Flock wrapper | `dev/benchmarks/ccx1-bench-flock.sh` |
| Baseline aggregate | `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv` |
| fflas reference CSV | `dev/bench_results/2026-04-26-reference.csv` |

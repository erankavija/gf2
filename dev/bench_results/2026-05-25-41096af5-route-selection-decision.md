# GF(251) Phase 1 Route-Selection Decision — issue 41096af5

| Field | Value |
|---|---|
| Date | 2026-05-25 |
| JIT issue | `41096af5` (Select and wire the GF(251) production route — 615db3b9 Phase 1 fan-in) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Predecessors | `68cdf4c8` (route A), `91429c1c` (route B), `fc182ed5` (route C) — all done |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2+FMA, no AVX-512 |
| Reference | fflas-ffpack 2.5.0 (pinned baseline from `cc5de315` closure) |
| Toolchain | rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1 |

---

## 1. Decision Rule (verbatim from issue description)

> - If optional BLAS gives a decisive win but the in-house routes stay below the 0.667 ratio, record BLAS as an optional accelerator/reference lane and keep the default build self-contained.
> - If in-Rust f32 or integer panelized kernels clear GF(251) within 1.5x of fflas at n in {256, 1024}, prefer the self-contained route for production.
> - If neither clears the threshold, split a focused architecture issue before continuing broad downstream work.

---

## 2. Per-Route Evidence Summary Table

fflas-ffpack reference at GF(251): n=256 → **128.48 Gop/s**, n=1024 → **138.32 Gop/s**
(Source: `dev/bench_results/2026-04-26-reference.csv`, confirmed in
`dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 5.)

Threshold (1.5× of fflas = ratio ≥ 0.667): n=256 → 85.65 Gop/s, n=1024 → 92.21 Gop/s.

| Route | n | GF(251) Gop/s | Ratio vs fflas | Status | Evidence doc |
|---|---:|---:|---:|---|---|
| A (in-Rust f32/FMA cascade) | 64 | — | — | not measured | `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` |
| A (in-Rust f32/FMA cascade) | 256 | 70.21 | 0.547 | **SHORTFALL** | `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` |
| A (in-Rust f32/FMA cascade) | 1024 | **93.90** | **0.679** | **PASS** (≥ 0.667) | `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` |
| B (BLAS/OpenBLAS sgemm, full) | 64 | 16.57 | — | not measured vs fflas | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| B (BLAS/OpenBLAS sgemm, full) | 256 | 35.12 | 0.273 | **SHORTFALL** | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| B (BLAS/OpenBLAS sgemm, full) | 1024 | 66.56 | 0.481 | **SHORTFALL** | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| B (BLAS/OpenBLAS sgemm, canon) | 64 | 40.71 | — | not measured vs fflas | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| B (BLAS/OpenBLAS sgemm, canon) | 256 | 49.62 | 0.386 | **SHORTFALL** | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| B (BLAS/OpenBLAS sgemm, canon) | 1024 | 78.26 | 0.566 | **SHORTFALL** | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| C (pure-integer Goto/BLIS panel) | 64 | 27.56 | — | **REGRESSION** vs Candidate C | `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.md` |
| C (pure-integer Goto/BLIS panel) | 256 | 64.16 | 0.499 | **SHORTFALL** | `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.md` |
| C (pure-integer Goto/BLIS panel) | 1024 | 74.65 | 0.540 | **SHORTFALL** | `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.md` |

Candidate C (current production default) at GF(251):
- n=64: 33.22 Gop/s — no fflas baseline at n=64
- n=256: 71.27 Gop/s (0.555 of fflas) — SHORTFALL; same-session default phase from route-A bench
- n=1024: 75.40 Gop/s (0.545 of fflas) — SHORTFALL; same-session default phase from route-A bench

(See route-A evidence doc § 3.2, "default" rows, for the same-session Candidate C numbers.)

---

## 3. Decision-Rule Branch Analysis

**Branch 1** — "If optional BLAS gives a decisive win but the in-house routes stay below 0.667":
- Route B (BLAS) at n=1024: full=0.481, canon=0.566. **Does NOT fire** — BLAS is itself below 0.667.
- Route B is not a "decisive win" over in-house routes; route A outperforms route B at both cells.

**Branch 2** — "If in-Rust f32 or integer panelized kernels clear GF(251) within 1.5x of fflas at n in {256, 1024}":
- Route A at n=1024: 0.679 of fflas. **PASS at n=1024**.
- Route A at n=256: 0.547 of fflas. **SHORTFALL at n=256**.
- Route C at n=256: 0.499. Route C at n=1024: 0.540. Both **SHORTFALL**.
- **Partially fires** — only route A at n=1024 clears; n=256 does not for any route.

**Branch 3** — "If neither clears the threshold":
- Would fire under a strict reading (no route clears BOTH cells). WOULD fire.

---

## 4. User-Approved Hybrid Resolution

Under strict reading, Branch 3 fires and the decision rule requires escalation before splitting a focused architecture issue. The lead escalated to the user on 2026-05-25. The user approved option 1:

> **Wire route A as default for GF(251)/n>=512; keep Candidate C for n<512; amend GF(251)/n=256 to [aspirational] (the 7a106fe4 evidence doc already marks GF(251) [aspirational] family-wide). Route B research-only (already in dev/research/blas_sgemm_gf251/, do not touch). Route C dormant behind `set_route_c_gf251_enabled` (do not touch — leave the AtomicBool toggle exactly as fc182ed5 left it).**

Source: `dev/active/026fc832-handoff-4.md` § "What just happened" (lines 25 and 34–36).

Rationale for the hybrid: route A is the only Phase-1 prototype that clears 1.5× of fflas at any GF(251) cell (n=1024, ratio 0.679). The pack-cost overhead amortises at n ≥ 512 (≈ 7% at n=1024 vs ≈ 28% at n=256 — see route-A evidence § 3.3 structural decomposition). Wiring route A for n ≥ 512 captures the available win while leaving the n=256 cell on Candidate C (the better option at that size). The GF(251)/n=256 criterion is amended to `[aspirational]` per the 7a106fe4 evidence doc's family-wide designation.

---

## 5. Selected Route and Reason

**Selected route: Route A** (in-Rust f32/FMA cascade, reworked Candidate F)

**Reason:** Route A is the only Phase-1 prototype that clears 1.5× of fflas at a GF(251) cell on the Zen-3 reference host. The partial-PASS (n=1024, ratio 0.679 ≥ 0.667) is pinned at n ≥ 512 where pack-cost overhead amortises. No other route (B, C) exceeds 0.667 at any cell.

**n=256 status:** [aspirational] — Candidate C (production default for n<512) delivers 71.27 Gop/s (0.555 of fflas). Route A delivers 70.21 Gop/s — slightly below Candidate C. The GF(251)/n=256 cell stays on Candidate C. The 7a106fe4 evidence doc marks GF(251) [aspirational] family-wide; this wire-in formalises that in the production dispatch rule.

---

## 6. Production Change Summary

**File changed:** `crates/gf2-core/src/gfp/simd_ops.rs`

**Function changed:** `select_f32_path<const P: u64>(_m, _k, n)`

- Before: always returned `false` for all in-scope primes (N_THRESH_PRIME=252 made `P >= 252 && P <= 251` impossible).
- After: `N_THRESH_PRIME` updated to 251; `select_f32_path` refactored to a single expression `P >= N_THRESH_PRIME && P <= 251 && n >= 512`. With N_THRESH_PRIME=251, this evaluates to `true` only for `P == 251 && n >= 512`.

**Dispatch changed:** `fp_small_try_gemm_classical` guard for the route-A code block is `if route_a_selected || (f32_selected && P == 251)`. The `f32_selected` flag is now the single source of truth for "GF(251)/n>=512 production default"; the `&& P == 251` guard is belt-and-suspenders (compile-time const-generic check, optimised out). This routes through the reworked route-A code (from_mont_f32 lookup-table pack + vectorized AVX2 Barrett reduction), NOT the legacy Candidate F path.

**Other primes affected:** NONE. GF(7), GF(31), GF(127), GF(241) all have `P < 251` so `P >= N_THRESH_PRIME` evaluates to `false` for them; they remain on Candidate C.

**AtomicBool toggle preserved:** `set_route_a_gf251_enabled(true)` continues to force route A for GF(251) at any n (override for testing/benching at n<512). The new production dispatch is the default-path (toggle=false) behaviour.

---

## 7. Post-Wire-In 5-Trial CCX1-Pinned Measurements

**Bench driver:** `dev/bench_results/run_41096af5_post_wire_in_bench.sh`

**CSV (raw):** `dev/bench_results/2026-05-25-41096af5-post-wire-in.csv`

**CSV (aggregate):** `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv`

### 7.1 Aggregate (5-trial median / Q1 / Q3 / IQR) — production default dispatch

Source CSV: `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv`

| prime | n | route | median Gop/s | Q1 | Q3 | IQR | min | max | vs fflas | verdict |
|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---|
| 7 | 64 | production_default | 32.156 | 31.970 | 32.169 | 0.199 | 31.089 | 32.184 | — | non-regression control |
| 7 | 256 | production_default | 70.746 | 70.733 | 70.804 | 0.072 | 70.707 | 71.158 | — | non-regression control |
| 7 | 1024 | production_default | 75.791 | 75.606 | 75.930 | 0.324 | 75.154 | 76.895 | — | non-regression control |
| 31 | 64 | production_default | 31.482 | 31.454 | 31.559 | 0.104 | 29.743 | 31.604 | — | non-regression control |
| 31 | 256 | production_default | 69.996 | 69.965 | 70.007 | 0.042 | 69.639 | 70.046 | — | non-regression control |
| 31 | 1024 | production_default | 76.509 | 76.349 | 76.605 | 0.256 | 75.308 | 76.991 | — | non-regression control |
| 127 | 256 | production_default | 69.804 | 69.801 | 69.837 | 0.036 | 69.477 | 69.906 | — | non-regression control |
| 127 | 1024 | production_default | 76.191 | 75.566 | 76.671 | 1.106 | 75.264 | 77.124 | — | non-regression control |
| 251 | 64 | production_default | 31.521 | 31.482 | 31.525 | 0.043 | 31.475 | 31.542 | — | Candidate C (n<512); non-regression control |
| **251** | **256** | **production_default** | **69.926** | 69.867 | 69.933 | 0.066 | 68.983 | 70.415 | **0.544** | **Candidate C (n<512); [aspirational] vs fflas** |
| **251** | **1024** | **production_default** | **94.425** | 94.377 | 94.842 | 0.464 | 93.948 | 95.829 | **0.683** | **Route A; PASS (≥ 0.667)** |

**Key result:** GF(251)/n=1024 routes through route A at 94.425 Gop/s (ratio 0.683, above the 0.667 threshold). This matches the pre-wire-in route-A measurement (93.90 Gop/s, ratio 0.679) within bench noise. The dispatch wire-in is mechanically equivalent to the pre-wire-in explicit-toggle path.

### 7.2 Non-regression check

Reference baselines (from `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` § 4.1 same-session medians):

| prime | n | baseline Gop/s | post-wire-in Gop/s | delta | verdict |
|---:|---:|---:|---:|---:|---|
| 7 | 256 | 44.65 | 70.746 | +58.5% | PASS (positive — contaminated session baseline; improvement confirmed) |
| 7 | 1024 | 75.42 | 75.791 | +0.5% | PASS |
| 31 | 256 | 70.92 | 69.996 | −1.3% | PASS |
| 31 | 1024 | 75.43 | 76.509 | +1.4% | PASS |
| 127 | 256 | 70.90 | 69.804 | −1.5% | PASS |
| 127 | 1024 | 75.32 | 76.191 | +1.2% | PASS |
| 251 | 256 | 71.27 | 69.926 | −1.9% | PASS (Candidate C; within 5%) |
| 251 | 1024 | 75.40 | 94.425 | +25.2% | PASS (route A active; large positive delta vs prior Candidate C) |

All non-GF(251)/n=1024 cells: delta ≤ 5% in absolute value — non-regression criterion PASS. The GF(7)/n=256 positive delta (+58.5%) reflects the uncontaminated baseline in the current session vs the partially-contaminated route-A bench session (§ 5 of route-A evidence doc documents the 2026-05-24 session contamination). This is not a regression.

**Non-regression: PASS on all 8 control cells.**

---

## 8. Bit-Exact Correctness

The following proptests verify production-dispatch correctness against the scalar naive GEMM reference:

### 8.1 Proptest file: `crates/gf2-core/tests/route_a_gf251_production_dispatch_proptests.rs`

| Proptest name | Shape (m, k, n) | n range | Primes | Route (production default) | Status |
|---|---|---|---|---|---|
| `proptest_production_dispatch_boundary_n_values` | m=k=n (square) | {0, 1, 15, 16, 17, 63, 64, 65} | GF(251) | Candidate C (n < 512) | PASS |
| `proptest_production_dispatch_n512_matches_scalar` | (4, 64, 512) | n=512 | GF(251) | route A (n ≥ 512) | PASS |
| `proptest_production_dispatch_n1024_matches_scalar` | (4, 64, 1024) | n=1024 | GF(251) | route A (n ≥ 512) | PASS |
| `proptest_production_dispatch_prime_sweep_boundary_n` | m=k=n (square) | {0, 1, 15, 16, 17, 63, 64, 65} | GF(7), GF(31), GF(127), GF(241), GF(251) | Candidate C (n < 512) | PASS |

All four proptests use `proptest!` macro blocks. The boundary-n blocks use `prop_oneof![Just(0), ...]` (per SC#9 / 52cce970 R1 review requirement). The n=512 and n=1024 blocks use rectangular shapes to stay within the 5-second CI ceiling. The prime-sweep block verifies that the N_THRESH_PRIME=251 wire-in does not affect correctness for GF(7)/GF(31)/GF(127)/GF(241) at boundary lengths (all n < 512 so all stay on Candidate C).

### 8.2 Existing route-A parity tests (unchanged, still PASS)

From `crates/gf2-core/tests/route_a_gf251_parity.rs`:
- `route_a_matches_default_at_criterion_n_values` — n ∈ {1, 15, 16, 17, 63, 64, 65, 255, 256, 257, 1023, 1024} PASS
- `route_a_matches_default_at_k_chunk_boundary` — k ∈ {1, 64, 256, 267, 268, 269, 512, 1023, 1024, 1025} PASS
- `route_a_matches_default_at_m_partial` — m ∈ {1, 2, 3, 5, 6, 7, 9, 33} PASS
- `route_a_matches_default_at_n_partial` — n ∈ {1, 8, 23, 24, 25, 47, 48, 49, 95, 96, 97, 121} PASS
- `route_a_off_leaves_dispatch_unchanged` — toggle-restore sanity PASS

All tests use AtomicBool-based toggle (no unsafe env-var mutation).

### 8.3 Gate: `cargo nextest run --workspace --all-features --release --profile ci`

Result: **3842 tests run, 3842 passed, 176 skipped** (including all new proptests above).

---

## 9. Source Index

| Reference | Path |
|---|---|
| Phase 1 plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` |
| User-approved decision source | `dev/active/026fc832-handoff-4.md` § "What just happened" |
| Route A evidence doc | `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` |
| Route B evidence doc | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| Route C evidence doc | `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.md` |
| Phase 0 baseline | `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` |
| Dispatch site | `crates/gf2-core/src/gfp/simd_ops.rs` (`select_f32_path`, `fp_small_try_gemm_classical`) |
| Bench site | `crates/gf2-core/benches/fieldmatrix_gemm.rs` (`bench_gemm_fp_251`) |
| Post-wire-in bench driver | `dev/bench_results/run_41096af5_post_wire_in_bench.sh` |
| Post-wire-in raw CSV | `dev/bench_results/2026-05-25-41096af5-post-wire-in.csv` |
| Post-wire-in aggregate CSV | `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv` |
| New proptests | `crates/gf2-core/tests/route_a_gf251_production_dispatch_proptests.rs` |

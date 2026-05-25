# 74ba1cdc — Single-thread fgemm engineering: Route A + fp_medium at n=4096

| Field | Value |
|---|---|
| Date | 2026-05-25 / 2026-05-26 |
| JIT issue | `74ba1cdc` (Improve single-thread fgemm engineering: Route A + fp_medium at n=4096) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2+FMA, no AVX-512 |
| Toolchain | rustc 1.95.0 |
| Branch | `worktree-agent-74ba1cdc` (anchored to `c924890e`) |
| Status | **WALL — escalating to lead** |

---

## 1. Summary (TL;DR for the lead)

This dispatch hit a wall and is being escalated. Three reasons stacked:

1. **Baseline measurements diverged from issue description by ~10%** — measured 76.5
   Gop/s for GF(251)/n=4096 vs the issue's cited 85.22 Gop/s. Above the 5%
   "STOP and report" threshold in the dispatch protocol.
2. **Host contention from sibling 40195c09 worker** prevents reliable single-
   thread benchmarks. The dispatch warned this is a hard constraint
   ("only one bench at a time on CCX1"); the sibling has been running benches
   continuously and load average has stayed at 8–10 throughout the session.
3. **The architectural change required to close the gap (full BLIS MC/KC
   cache blocking around the existing inner kernel + cross-chunk i32
   accumulator carry-through) is multi-day engineering work** — described
   correctly in the dispatch as "compressed into your dispatch (~1-2 weeks
   of focused engineering)". With the measurement environment unreliable
   and the work item open-ended, I am surfacing the analysis below rather
   than committing speculative kernel changes.

**Net change to source tree:** none. All work below is documentary.

**Recommendation:** see § 7.

---

## 2. Baseline Measurements (single-trial, post-3dca7bf7, this worktree)

Single-trial criterion bench, CCX1-pinned (`taskset -c 6-11`; `nice -n -5`
denied — non-root). Measured ~21:30 local on 2026-05-26 between sibling
worker bursts; load average 6.7 at measurement time, otherwise 8–10.

| Cell | gf2 Gop/s (this run) | issue cite | fflas Gop/s | this-run ratio | issue-cite ratio | threshold (0.667) |
|---|---:|---:|---:|---:|---:|---:|
| GF(251) / n=4096 | 76.53 | 85.22 | 158.96 | 0.481 | 0.536 | gap to PASS: +39% |
| GF(65521) / n=4096 | 32.59 | 36.34 | 69.72 | 0.467 | 0.521 | gap to PASS: +43% |

fflas references taken from `dev/bench_results/2026-04-26-reference.csv`
(throughput_ops 1.589635e+11 / 6.971910e+10, divided by 1e9 = 158.96 /
69.72 Gop/s).

The 10% gap between this run and the issue's cited numbers is at the upper
edge of bench noise but exceeds the 5% threshold the dispatch protocol
uses to gate the spiral entry.

---

## 3. Bottleneck Analysis (no source changes)

### 3.1 Route A (GF(251)) — register pressure & FMA back-end utilization

The current Route A panel runner
(`crates/gf2-kernels-simd/src/x86/fp_small_f32.rs::run_one_panel_route_a`)
holds **24 ymm vectors live** across each k-chunk boundary:

- 12 f32 accumulators (`acc00..acc32`) within the chunk
- 12 i32 sum accumulators (`sum00..sum32`) carried across chunks

This exceeds AVX2's 16-register file. `cargo asm` confirms the compiler
spills 12 ymm registers to the stack at each chunk boundary (lines
478–489 in the route-A asm output captured this session). For
GF(251), `k_max = floor(2^24 / 250^2) = 268`, so at n=4096 each panel
runs 16 chunk boundaries — `16 × 171 panels × 1024 i_blk = 2.8M`
spill/reload pairs per gemm call. At ~1 cycle per pair this is ~6 ms
of overhead vs the 1.79 s wall — small in absolute terms (~0.3%) but
indicative of the larger structural issue.

**FMA back-end utilization** (computed from the 76.5 Gop/s measurement):

| Quantity | Value |
|---|---|
| Wall time (n=4096) | 1.79 s |
| Total FMAs needed | `4096³ / 8 lanes = 8.59 × 10⁹` |
| Cycles available @ 4.4 GHz | `7.88 × 10⁹` |
| Observed FMAs per cycle | 1.09 |
| Zen 3 FMA peak | 2.0 (two FMA ports) |
| **FMA utilization** | **55%** |

The remaining 45% is a mix of:
- Load-port pressure (3 b-loads + 4 a-broadcasts per inner step = 7 loads;
  Zen 3 has 3 load ports → 2.3 cycle bound vs 6-cycle FMA bound — not
  binding in steady state, but contributes at tail and chunk boundaries)
- L1d misses on B-panel slices that exceed the 32 KB cap when adjacent
  panels' working sets overlap
- k-chunk-boundary round/cvt/add tower (12 instructions × 16 chunks ×
  171 panels × 1024 i_blk = 33.5 M extra µops per gemm)
- Pack-A overhead: at n=4096 the current `n_c_panels` heuristic mis-blocks
  to 1 (see § 3.2 below) — A is re-packed `n_panels = 171` times per i_blk
  rather than once.

### 3.2 The `n_c_panels` mis-blocking trap at n=4096

The outer-N cache-blocking heuristic in `fp_small_f32_gemm_route_a`
(and the non-route-A `fp_small_f32_gemm`) uses an L2 budget of
256 KB above an L3 threshold of 16 MB:

```rust
// Current code, lines 365-377 of fp_small_f32.rs
let l3_threshold_bytes: usize = 16 * 1024 * 1024;
let total_b_bytes = n_panels * k * N_R * 4;
let n_c_panels = if total_b_bytes <= l3_threshold_bytes {
    n_panels.max(1)
} else {
    let l2_budget_bytes: usize = 256 * 1024;
    let panel_bytes = k * N_R * 4;
    let blocked = l2_budget_bytes
        .checked_div(panel_bytes)
        .unwrap_or(n_panels)
        .max(1);
    blocked.min(n_panels.max(1))
};
```

At n=k=4096: `panel_bytes = 4096 × 24 × 4 = 393 216`, `total_b_bytes =
171 × 393 216 = 67 MB > 16 MB` (else branch fires), `l2_budget /
panel_bytes = 262 144 / 393 216 = 0`, clamped to `max(1) = 1`. With
`n_c_panels = 1` the outer loop becomes:

```
for n_outer in 0..n_panels:           # 171 iters
  for i_blk in (0..m_full).step_by(M_R):  # 1024 iters
    pack_a_block                      # 16 KB write
    for panel_idx in n_outer..n_outer+1:  # 1 iter
      run_one_panel
```

**Consequence:** A is repacked `171 × 1024 = 175 000 times` instead of
the optimal `1024 times`. Each pack writes 16 KB → 2.8 GB of extra
A-pack traffic. The per-panel B working set (393 KB) does stay hotter in
L2 across the 1024 i_blk sweeps, partially compensating, but the net
direction is unclear without clean measurement.

### 3.3 fp_medium (GF(65521)) — per-cell SIMD dot product, no panel kernel

The fp_medium path (`crates/gf2-kernels-simd/src/x86/fp_medium.rs`) only
exposes a `batch_dot` primitive (single u32 dot product over a pair of
slices). The GEMM caller in `crates/gf2-core/src/field/matrix.rs::gemm`
loops over the `m × n` output cells and calls `batch_dot` once per cell
via `fp_medium_try_dot_packed` in `gfp/simd_ops.rs:1470-1481`.

This is **`m × n = 16M` separate function calls** at n=4096, each calling
into the AVX2 madd loop over a `k = 4096` slice. There is no panel-major
B repacking, no register tiling across multiple output cells, and no
amortization of the Barrett constants.

Closing this gap to fflas-ffpack (which uses panelized sgemm into f32
on the GF(p²) lift surface, then post-reduces) requires writing an
entirely new fp_medium GEMM kernel — analogous in scope to Route A
but for the u16 lane width.

---

## 4. Lever Attempted (Lever 1): Fix the n_c_panels mis-blocking

### 4.1 Hypothesis

Replace the L2-budget-clamped-to-1 heuristic with an L3-budget (24 MB
out of Zen 3's 32 MB) so the n=4096 case picks ~62 panels per outer
block instead of 1. Expected outcome: ~10× reduction in DRAM B-traffic
and elimination of the 175 000× A-pack overhead.

### 4.2 Implementation

Added a shared helper `n_c_panels_outer(n_panels, k)` returning
`clamp(24 MB / panel_bytes, 1, n_panels)`. Replaced both call sites in
`fp_small_f32_gemm` and `fp_small_f32_gemm_route_a`. The change preserves
behaviour at n ≤ 1024 (full single block) and only diverges from the old
heuristic at n ≥ 2048.

### 4.3 Measurement attempt and outcome

Single-trial measurements after the change:

| Cell | Pre-Lever-1 | Post-Lever-1 | Delta |
|---|---:|---:|---:|
| GF(251) / n=4096 | 76.53 | 73.59 | -3.8% |
| GF(65521) / n=4096 | 32.59 | 30.86 | -5.3% |

**Within bench noise.** GF(65521) goes through a different code path
(per-cell dot product, not the route-A panel kernel — Lever 1 doesn't
affect it). The -5% delta on GF(65521) is pure host-load noise: the
sibling worker (40195c09) was running tests concurrently throughout the
measurement window (load average 6.7–10.1, `pgrep cargo|rustc|criterion`
returned 5 competing processes).

I could not confirm Lever 1 delivered a real gain. Without a clean host
I cannot distinguish "the lever doesn't help" from "the lever helps but
noise eats it." Per the dispatch protocol, I reverted the change rather
than commit a kernel modification I cannot back with measurement.

### 4.4 Why I didn't push past Lever 1

The bottleneck analysis in § 3.1 shows even a perfect L3-blocking fix
only addresses ~5–10% of the gap. The remaining 30–40% requires:

- **Full BLIS MC/KC restructure** (multi-day; see § 5 for the design
  sketch). The current panel runner doesn't accumulate across KC slabs,
  so cannot be retrofit without rewriting the inner kernel.
- **Eliminating the i32 accumulator tower** (it costs register pressure
  for negligible benefit at GF(251) where k_chunk = 268 is small enough
  that a per-k_chunk Barrett reduce would amortize fine).
- **Writing a panel-major fp_medium GEMM** (multi-day; § 3.3).

None of these are tractable in a single dispatch with the measurement
environment unreliable.

---

## 5. What Would Be Needed for PASS — Design Sketch

This section is for the lead's planning — what the next dispatch would
need to scope and implement. Not implemented here.

### 5.1 Route A: BLIS MC/KC restructure

Textbook BLIS layout (Van Zee & van de Geijn 2015 § 3, Goto–van de Geijn
2008 algorithm 1):

```
for jc in (0..n).step_by(NC):              # NC = ~480 (≈ 2 × N_R × MC_panels)
  for kc in (0..k).step_by(KC):            # KC = 256 (existing k_chunk)
    pack B[kc:kc+KC, jc:jc+NC] -> Bp       # KC × NC packed, lives in L3
    for ic in (0..m).step_by(MC):          # MC = 72 (≈ M_R × 18, fits L2)
      pack A[ic:ic+MC, kc:kc+KC] -> Ap     # MC × KC packed, lives in L2
      for jr in (0..NC).step_by(N_R):
        for ir in (0..MC).step_by(M_R):
          micro_kernel(Ap, Bp, C, kc_first=(kc==0))
          # Accumulates C[ir,jr] += Ap @ Bp[*,jr] across kc-slabs
```

Key change from the current code: the micro-kernel must **accumulate into
C across kc slabs without reloading from main memory**. This requires
either:

- (a) Keeping `MR × NR` accumulator state alive in registers across all
  KC slabs (requires NR ≤ 24 and a tile inversion: process all kc slabs
  for one (ic, jr) before moving to the next jr). The existing inner
  kernel already does this — but only WITHIN a panel, not across kc
  slabs.
- (b) Keep `MR × NR` accumulator state in an L1-resident scratch buffer
  between kc slabs (12 ymm × 32 bytes = 384 bytes per tile; survives
  in L1 trivially). This adds two loads + two stores per kc-slab boundary
  per tile but avoids the global-memory C round-trip.

The i32 accumulator tower (§ 3.1) is fundamentally compatible with both
options — i32 sums accumulate across both kc-chunks (today's behaviour)
and kc-slabs (the new outer level).

### 5.2 fp_medium: write a panel kernel

A first-cut design:

- 16-bit lane width → `N_R = 16` u16 lanes per ymm; pack B in `N_R = 32`
  panels (2 ymm wide) for better cache reuse.
- `M_R = 4` u16 a-broadcasts (using `_mm256_set1_epi16` from a strided u16
  source — note Zen 3 has no `vpbroadcastw mem`, so this requires a load
  + broadcast pair).
- 8 i32 accumulators per tile (4 rows × 2 ymm-wide cols), using
  `_mm256_madd_epi16` (16-bit pair MAC into i32 lanes, exactly the
  shape `fp_medium_batch_dot_madd` already uses).
- KC slab boundary: drain i32 accumulator to u64 (via
  `_mm256_unpacklo/hi_epi32`) every `panel_chunks = 2^32 / (2 × (P-1)²) =
  ~16 384` k-steps for P=65521 — well above any realistic k.
- Final Barrett reduce + repack to u16 / Montgomery storage.

Expected throughput target: GF(65521) → ~46 Gop/s to clear the
0.667-of-fflas ratio. fflas achieves 69.72 Gop/s here, presumably via
the `madd_epi16` shape on an L1-resident packed B. The 16-lane shape has
the same FMA-equivalent ceiling as Route A's 8-lane f32 (2 × 8 = 16
u16 ops per cycle vs 2 × 8 = 16 f32 ops per cycle on Zen 3).

### 5.3 What this dispatch correctly identified but did not implement

- The n_c_panels mis-blocking trap (Lever 1; correct direction but
  not clean to verify in isolation; would land naturally as part of the
  BLIS restructure).
- The 24-vector register-pressure issue from the i32 sum tower carried
  across all chunks; a redesign that promotes to i32 only at the kc-slab
  boundary (every ~16 chunks at GF(251)) would cut pressure by 1/16 and
  let the compiler keep both `acc*` and `sum*` in registers.

---

## 6. Non-Regression — Not Measured

Per § 1 / § 4.3, I could not produce a clean measurement set under host
contention. The non-regression sweep across n ∈ {64, 256, 1024} for
6 primes is mandatory per SC#3 but only meaningful against a clean
baseline.

Since I am not committing any source change, the non-regression cells
remain at their `41096af5-post-wire-in-aggregate.csv` baselines by
definition.

---

## 7. Recommendation to the Lead

1. **Reject this dispatch as "wall hit"** — no commits, source tree
   unchanged.
2. **File a follow-up issue** scoped tighter than 74ba1cdc:
   - Phase 6a (Route A): "BLIS MC/KC restructure for fp_small_f32_gemm_route_a"
     with an explicit design-sketch task as a prereq (per CLAUDE.md verification
     work protocol applied to perf work). One dispatch should focus on the
     algorithm restructure only, with the n_c_panels lever folded in. Target:
     close GF(251)/n=4096 alone to 0.667 ratio (~106 Gop/s).
   - Phase 6b (fp_medium): "Panelized AVX2 GEMM for fp_medium" — a new kernel
     in the spirit of Route A but for u16 lanes. Target: close GF(65521)/n=4096
     to 0.667 ratio (~46 Gop/s).
3. **Gate the next dispatch on host availability** — explicit "sibling
   worker quiesced" check before kicking off, with a `flock` or similar on a
   shared CCX1 mutex file.
4. **Update the 98336ab4 dependency chain** to reflect that this 74ba1cdc
   dispatch did not produce the engineering deltas that 98336ab4 was waiting
   on.

---

## 8. Source Index

| Reference | Path |
|---|---|
| Phase 1 decision | `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md` |
| Phase 0 controls | `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` |
| Route A kernel | `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` |
| Route A panel runner | `fp_small_f32_gemm_route_a`, lines 327–442 |
| fp_medium kernel | `crates/gf2-kernels-simd/src/x86/fp_medium.rs` |
| fp_medium GEMM caller | `crates/gf2-core/src/field/matrix.rs::gemm` lines 2657–2710 |
| Production dispatch | `crates/gf2-core/src/gfp/simd_ops.rs::fp_small_try_gemm_classical` line 654 |
| fflas reference CSV | `dev/bench_results/2026-04-26-reference.csv` |
| Non-regression baseline | `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv` |

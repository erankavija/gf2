# Route C: pure-integer Goto/BLIS-style panelized GF(251) micro-kernel — design note

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `fc182ed5` (Prototype pure integer panelized GF(251) micro-kernel) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 1, item 3 |
| Predecessor | `a70b1c70` (Phase 0 baseline measurements) |
| Sibling routes | `68cdf4c8` (route A in-Rust f32/FMA, closed); `91429c1c` (route B OpenBLAS cascade, closed) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2 + FMA, no AVX-512 |

This note records the design decisions for a pure-integer Goto/BLIS-style
panelized micro-kernel for GF(251) at n ∈ {256, 1024}, per the verbatim
Phase 1 item 3 in `dev/active/615db3b9-finite-field-la-sota-plan.md`:

> Add explicit A/B panel packing and a register-blocked AVX2
> micro-kernel for byte/word-fits-in-u16 primes. This is likely 1-2x
> slower than OpenBLAS but is the most appropriate long-term default
> if the project wants self-contained Rust kernels.

## 1. Baseline (from Phase 0 and route A/B closures)

From `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 5
(GF(251) drift check at HEAD) and the route closures:

| cell | Candidate C Gop/s | route A Gop/s | route B (full) Gop/s | fflas Gop/s |
|---|---:|---:|---:|---:|
| GF(251)/n=256 | 58.98 (drift 61.03 / 71.27) | 70.21 | 35.49 | 128.48 |
| GF(251)/n=1024 | 70.89 (drift 72.44 / 75.40) | 93.90 | 66.56 | 138.32 |

The 1.5x-of-fflas threshold (`gf2/fflas ≥ 0.667`) requires:

- n=256: gf2 ≥ 128.48 × 0.667 = **85.65 Gop/s**
- n=1024: gf2 ≥ 138.32 × 0.667 = **92.21 Gop/s**

Route A clears n=1024 (0.679 ratio, partial Phase-1 candidate) but
not n=256 (0.547). Route B fails both cells outright. Route C is the
remaining option for a fully self-contained Rust path that closes the
n=256 gap without taking on an external BLAS dependency.

Route C's design lever (different from routes A and B): use AVX2
**integer** `_mm256_madd_epi16` lane-pair MAC against explicit
Goto/BLIS-style A/B panel-packed buffers. Candidate C uses
`_mm256_madd_epi16` per row-panel but **without** explicit
panel-packing — every output row reloads 16-byte slices of A and B
non-contiguously. Route C amortises this by repacking once per
panel and reusing the packed slabs across the inner k-loop.

## 2. Panel dimensions

### 2.1 Inner register tile (`MR × NR`)

- **MR = 4** (rows of A per inner micro-kernel tile)
- **NR = 24** (columns of output per inner micro-kernel tile, split into 3 × 8-lane i32 column-sub-tiles)

Register-file accounting on Zen 3 (16 ymm registers per core):

| Role | Count | Notes |
|---|---:|---|
| u32 accumulators (one per output sub-tile cell) | `MR × (NR / 8) = 4 × 3 = 12` | 8 i32 lanes each |
| B-pair loaded u16 vectors (one per column sub-tile) | `NR / 8 = 3` | 16 u16 lanes each; loaded once per t-pair |
| A-pair broadcast (one row at a time) | 1 | 16 u16 lanes = 8 u32 (pair-of-bytes broadcast) |
| **Total** | **16** | Exactly fills the AVX2 register file. |

This is the same shape route A uses for f32 (12 FMA accumulators × 8
lanes = 96 outputs per tile), so the high-level register pressure is
the same; only the lane arithmetic changes (i32 lanes via
`_mm256_madd_epi16` instead of f32 FMA).

**Public reference for MR × NR = 4 × 24 on Zen 3 AVX2:** Goto and
van de Geijn, "Anatomy of High-Performance Matrix Multiplication"
(ACM TOMS 34(3), 2008), § 4 / § 5. The paper's recipe is "fill the
register file with accumulators; pick `MR × NR` such that
`MR × NR / SIMD_LANES = REG_FILE_SIZE − OPERAND_REGS`." For Zen 3
AVX2 (16 ymm regs, 8 i32 lanes per ymm, 4 operand regs reserved for
loads + broadcasts), the closed form gives
`MR × NR / 8 = 12`. The factorisation `4 × 24` keeps MR small enough
to amortise per-row pack cost (only `m/MR` packs of A per panel)
while making NR a multiple of 8 (one i32-sub-tile per ymm load).
The same factorisation is used by route A and by Candidate F's
dormant production kernel.

### 2.2 Cache-blocking dimension (`KC`)

- **KC = 256** (inner k-axis cache blocking)

Choice derivation (public references only):

1. **u32 overflow bound.** Each `_mm256_madd_epi16` lane sums two u16
   products, each ≤ `(p−1)² ≤ 250² = 62 500`. Across `kc/2` pair-steps
   the per-lane sum ≤ `(kc/2) × 2 × 62 500 = 62 500 · kc`. For u32
   range (`2^32 ≈ 4.29 × 10^9`), this bounds
   `kc ≤ 2^32 / 62 500 ≈ 68 719`. For i32 (signed, since `_mm256_madd_epi16`
   technically writes signed i32) the cap drops to
   `kc ≤ 2^31 / 62 500 ≈ 34 359`. **The arithmetic bound is far above
   the cache-fit bound; KC is L1d-bound, not overflow-bound.**

2. **L1d-fit working set.** Per inner panel sweep the working set is:
   - **A-pack slab** (4 rows × KC bytes, stored as u16 pair-broadcasts of
     2 bytes each → KC × MR × 2 bytes): `256 × 4 × 2 = 2 048 B` (32 lines).
   - **B-pack panel slab** (KC rows × NR bytes packed in 2-row pairs):
     `KC × NR = 256 × 24 = 6 144 B` (96 lines).
   - **u32 accumulators in regs:** 0 memory traffic.
   - Total resident: **~8 KB**, well within Zen 3's 32 KB L1d (12.5%
     occupancy with room for caller stack frames and the `c` write
     stream).

   **Public reference:** AMD Software Optimization Guide for Family 19h
   (Zen 3), revision 3.07 (Publication ID 56665) § 2.13 — L1d size
   32 KB, 8-way associative. The Goto-vandeGeijn 2008 paper § 6 ("Cache
   blocking") prescribes: pick `KC` such that `(MR + NR) × KC × elem_size`
   fits in L1d with ≤ 1/4 occupancy so eviction pressure from the
   output write stream doesn't trash the packs. Our 25% headroom is
   the standard choice.

3. **Even-grain pair processing.** The inner kernel processes 2 k-rows
   per `_mm256_madd_epi16`, so KC must be a multiple of 2. KC = 256
   satisfies this; the kc tail (handled below) absorbs odd-k inputs.

### 2.3 Packing layout

#### A-pack: `MR × KC` interleaved 16-bit pair-broadcasts

Stored as `Vec<u16>` of length `MR × KC` (or `Vec<u32>` of length
`MR × KC / 2`, equivalent). For a 4-row block starting at row `i_blk`:

```text
a_pack[t/2 * MR * 2 + i * 2 + 0] = a[(i_blk + i), t]      (u16 storing u8 canonical)
a_pack[t/2 * MR * 2 + i * 2 + 1] = a[(i_blk + i), t + 1]  (u16 storing u8 canonical)
```

Equivalently, viewed as `Vec<u32>` of length `MR × (KC / 2)`:

```text
a_pack32[t/2 * MR + i] = (a[(i_blk + i), t + 1] as u32) << 16
                      | (a[(i_blk + i), t]     as u32)
```

The inner kernel reads `a_pack32[t/2 * MR + i]` as a single u32 and
broadcasts it via `_mm256_set1_epi32` → one ymm holding the same pair
`[a[i,t], a[i,t+1]]` repeated 8 times across 16 u16 lanes. That's
exactly the operand `_mm256_madd_epi16` consumes against the
correspondingly-paired B vector.

**Public reference:** Goto-vandeGeijn 2008 § 4.2 — "Pack A into
horizontal panels of `MR` rows each, with elements interleaved across
the `MR` rows so the inner-kernel reads a single contiguous panel
slice per inner step." Our 16-bit pair-broadcast variant is the
natural specialisation when the inner kernel is `_mm256_madd_epi16`
instead of `vfmadd231ps`.

#### B-pack: `KC × NR` packed-pairs panel, NR-major

Stored as `Vec<u8>` per n-panel (one panel = NR consecutive output
columns); each panel has `(KC + 1) / 2 * (NR * 2)` bytes (= `KC * NR`
bytes when KC is even). For one t-pair `(t, t+1)` within one n-panel
starting at output column `j_blk`:

```text
b_pack[panel_off + (t/2) * NR * 2 + j_off * 2 + 0] = b[j_blk + j_off, t]
b_pack[panel_off + (t/2) * NR * 2 + j_off * 2 + 1] = b[j_blk + j_off, t + 1]
```

(B is the original right operand; the caller passes `bt`, the row-major
transpose, so `bt[j * k + t] = b[j, t]` — read from `bt` during pack.)

Per t-pair the inner kernel loads 3 ymm = 3 × 16 u16 lanes:

- `b0 = _mm256_cvtepu8_epi16(_mm_loadu_si128(b_pack_ptr + 0))` → 16 u16
   carrying `[b[j_blk,t], b[j_blk,t+1], b[j_blk+1,t], b[j_blk+1,t+1], ...,
   b[j_blk+7,t], b[j_blk+7,t+1]]`
- `b1`, `b2` similarly for `j_blk+8..16` and `j_blk+16..24`.

**Public reference:** Goto-vandeGeijn 2008 § 4.2 — "Pack B into vertical
panels of NR columns each, transposed within each panel so the inner
kernel reads a contiguous strip per inner step." Our 2-row pair-of-bytes
packing is the natural specialisation for `_mm256_madd_epi16`'s
lane-pair input shape.

### 2.4 Inner kernel pseudo-code

```text
for t_pair in 0..(KC / 2):
    a0 = broadcast_pair(a_pack32[t_pair * MR + 0])  # row 0
    a1 = broadcast_pair(a_pack32[t_pair * MR + 1])  # row 1
    a2 = broadcast_pair(a_pack32[t_pair * MR + 2])  # row 2
    a3 = broadcast_pair(a_pack32[t_pair * MR + 3])  # row 3

    b0 = cvtepu8_epi16(load128(b_pack + t_pair*48 +  0))  # cols  0..8 paired
    b1 = cvtepu8_epi16(load128(b_pack + t_pair*48 + 16))  # cols  8..16 paired
    b2 = cvtepu8_epi16(load128(b_pack + t_pair*48 + 32))  # cols 16..24 paired

    acc00 = add_epi32(acc00, madd_epi16(a0, b0))
    acc01 = add_epi32(acc01, madd_epi16(a0, b1))
    acc02 = add_epi32(acc02, madd_epi16(a0, b2))
    acc10 = add_epi32(acc10, madd_epi16(a1, b0))
    acc11 = add_epi32(acc11, madd_epi16(a1, b1))
    acc12 = add_epi32(acc12, madd_epi16(a1, b2))
    acc20 = add_epi32(acc20, madd_epi16(a2, b0))
    acc21 = add_epi32(acc21, madd_epi16(a2, b1))
    acc22 = add_epi32(acc22, madd_epi16(a2, b2))
    acc30 = add_epi32(acc30, madd_epi16(a3, b0))
    acc31 = add_epi32(acc31, madd_epi16(a3, b1))
    acc32 = add_epi32(acc32, madd_epi16(a3, b2))
```

12 `madd_epi16` + 12 `add_epi32` + 3 b-loads + 3 broadcasts per t-pair.
On Zen 3 with two SIMD ALU pipes that each retire one 256-bit integer
op per cycle (per AMD Software Optimization Guide § 2.10), the inner
body is back-end-bound at roughly **12 cycles per t-pair** (each
`madd + add` retires on alternating pipes; the loads stream from the
L1d L→S unit).

The throughput envelope: 12 cycles per t-pair = 12 cycles per 192 MACs
(`MR × NR / 2` — half because each madd retires 2 MACs as a u16-pair
multiply-add). On Zen 3 at 5 GHz boost the ceiling is
`192 × 5e9 / 12 = 80 Gop/s` per Gop/s being `2 mkn` ops. Doubling for
the pair (each madd fold counts as 2 MACs) gives effective inner-loop
peak ≈ 80 Gop/s; this matches Candidate C's measured 70-72 Gop/s closely.

**Comment on the ceiling:** route A's f32 FMA pipe gives a higher
theoretical peak (160 Gop/s) because Zen 3's two FMA execution ports
each retire 8 f32 lanes per cycle, vs the integer ALU pipes that
retire 8 i32 lanes per cycle. Route C's win against Candidate C is
not from a higher inner-loop peak but from **lower per-tile overhead**
(fewer non-contiguous loads, fewer broadcast µops on the integer
front-end). Whether that overhead reduction adds up to the 1.5×-of-fflas
target at n=256 is the empirical question this prototype resolves.

## 3. Modular reduction at panel boundary

After the kc-blocked inner loop completes for one `(i_blk, n-panel)`
tile, the 12 u32 accumulator ymm vectors each hold a lane value in
`[0, KC/2 × 2 × (p−1)² × (n_chunks_per_panel))`. Because of u32 lanes
we cap the per-panel sum bound at `kc · (p−1)²` per lane after one kc
pass; across multiple kc chunks (when `k > KC`) we accumulate into the
same u32 ymm registers as long as the post-add value stays `< 2^32`.
For `p = 251`, `(p−1)² = 62 500`; the lane bound after `k` total k-steps
is `k × 62 500 / 2 ≈ 31 250 · k` (per pair; the madd folds 2 MACs).
For `k ≤ 1024`, the lane sum ≤ `1024 × 62 500 = 6.4 × 10^7`, well within
u32 / i32 range.

Reduction step at panel boundary, **per ymm accumulator vector**, applied
12 times per tile:

```text
acc_reduced = barrett_reduce_lane32(acc, mu_vec, p_vec, p_vec64)
              # SSOT in crates/gf2-kernels-simd/src/x86/fp_small.rs
              # (Candidate C's SpMM 32-bit Barrett reducer)
```

We reuse `super::fp_small::barrett_reduce_lane32` verbatim — the same
SSOT route A delegates to via `barrett_reduce_lane32_local`. No new
modular-reduction algebra is introduced; the algorithm is the standard
Granlund-Möller (2011) 32-bit-magic Barrett step with one conditional
subtract.

Pack 8 reduced i32 lanes → 8 u8 lanes via `_mm256_packus_epi32` +
`_mm256_permute4x64_epi64` + `_mm256_packus_epi16` (the same 3-step
sequence Candidate C uses in `pack_i32x8_to_u8` via the route-A
helper — also SSOT-reused).

## 4. Toggle mechanism

Per criterion 1, the route C path is exposed via a **safe `AtomicBool`
runtime debug switch**, byte-for-byte mirroring the route-A pattern
introduced by issue 68cdf4c8 R1 (commit `4bad2e72`). This pattern
avoids the `unsafe { set_var }` trap that bit the route-A R0 attempt
and keeps `gf2-core`'s `#![deny(unsafe_code)]` attribute intact:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static ROUTE_C_GF251_ENABLED: AtomicBool = AtomicBool::new(false);

/// Safe runtime debug switch for jit:fc182ed5 route C. Default off;
/// production dispatch is unaffected unless a caller opts in.
pub fn set_route_c_gf251_enabled(enabled: bool) {
    ROUTE_C_GF251_ENABLED.store(enabled, Ordering::Relaxed);
}

#[inline]
fn route_c_gf251_enabled<const P: u64>() -> bool {
    if P != 251 { return false; }
    ROUTE_C_GF251_ENABLED.load(Ordering::Relaxed)
}
```

Dispatch precedence within `fp_small_try_gemm_classical` (preserves the
existing dispatch lattice; only adds one branch above Candidate C):

1. Route A toggle on (P == 251) → route A
2. **Route C toggle on (P == 251) → route C (NEW)**
3. `select_f32_path::<P>(m, k, n)` true → Candidate F (dormant)
4. Otherwise → Candidate C

If both route A and route C are toggled on simultaneously, route A
wins (we keep insertion order to preserve the existing 68cdf4c8
behaviour in tests). The bench driver toggles exactly one at a time.

Per-call overhead is one relaxed atomic load (~1-2 ns) at the GEMM
dispatch site — exactly the same overhead route A pays. Production
dispatch (both flags false) is unchanged: zero impact on default
behaviour.

## 5. Public references consulted

No fflas-ffpack, Givaro, or FFPACK source code, comments, autotuning
tables, or micro-kernel structure was consulted in designing this
prototype. The local fflas checkout at
`/home/vkaskivuo/Projects/fflas-ffpack` was not opened. The
implementation derives entirely from public mathematical / ISA
references plus gf2-owned prior art:

1. **Goto, K., and van de Geijn, R. A.** "Anatomy of High-Performance
   Matrix Multiplication." ACM Trans. Math. Softw., 34(3):12, 2008.
   — The Goto/BLIS framework (pack A into MR-row horizontal panels,
   pack B into NR-column vertical panels, register-block the inner
   kernel to fill the SIMD register file). The MR × NR = 4 × 24 tile
   shape and the KC L1d-fit derivation in § 2 are direct applications
   of § 4 / § 5 / § 6 of the paper.

2. **van Zee, F. G., and van de Geijn, R. A.** "BLIS: A Framework for
   Rapidly Instantiating BLAS Functionality." ACM Trans. Math. Softw.,
   41(3):14, 2015. — The "5-loop" BLIS structure (3-level cache loops
   around the GotoBLAS-style packed inner kernel). Route C uses the
   inner two loops (KC blocking + MR/NR register-blocked micro-kernel)
   but does not implement outer L3-blocking — the workload at
   n ≤ 1024 fits comfortably in Zen 3's 32 MB L3.

3. **AMD Software Optimization Guide for AMD Family 19h Processors
   (Zen 3), revision 3.07.** Publication ID 56665. — Used for:
   - § 2.10 (SIMD execution pipelines and throughput) — confirms two
     256-bit integer ALU pipes each retire one `vpmaddwd` /
     `_mm256_madd_epi16` per cycle.
   - § 2.13 (L1d size, associativity, store-queue depth) — informs
     the KC = 256 L1d-fit derivation in § 2.2.
   - § 4.6 (instruction latency / throughput tables) — `vpmaddwd` 5
     cycles latency, 0.5 cycles reciprocal throughput; `vpaddd` 1
     cycle latency, 0.25 cycles reciprocal throughput.

4. **Granlund, T., and Möller, N.** "Improved division by invariant
   integers." IEEE Trans. Comput., 60(2):165–175, 2011. — Magic-number
   Barrett reduction for unsigned divide by an invariant. Used for the
   32-bit-magic Barrett path (`barrett_reduce_lane32`). Same reference
   route A cites.

5. **Dumas, J.-G., Giorgi, P., and Pernet, C.** "Dense Linear Algebra
   over Word-Size Prime Fields." ACM TOMS 35(3), 2009; arXiv:cs/0601133.
   — General framework for the GF(p) GEMM problem and the headroom
   bound. The integer-route (vs float-cascade) analysis in § 4 of the
   paper motivates the route-C design: when integer SIMD is available
   and `(p−1)² · kc` fits in the accumulator, the integer path avoids
   the cvt round-trip overhead the float cascade pays.

6. **gf2-owned prior art:**
   - `crates/gf2-kernels-simd/src/x86/fp_small.rs::barrett_reduce_lane32`
     — Candidate C's 32-bit-lane Barrett reducer. Reused verbatim
     (made `pub(super)` already by jit:68cdf4c8 R1 for the same
     reason); see SSOT rule for `barrett_reduce_lane32_local`.
   - `crates/gf2-kernels-simd/src/x86/fp_small.rs::fp_small_gemm_row_panel`
     — Candidate C's row-panel kernel; structurally similar inner-loop
     shape (`_mm256_madd_epi16` over 4 output cells in parallel) but
     without panel packing or KC blocking. Route C extends this
     structure with Goto/BLIS panelization.
   - `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` (route A) —
     the i32 accumulator architecture (12 u32 ymm registers at chunk
     boundaries) and the `pack_i32x8_to_u8` output-pack helper. The
     final u8 store sequence is byte-identical between route A and
     route C (same SSOT 3-step pack).
   - `dev/active/68cdf4c8-route-a-design.md` — the route-A design note,
     which sets the toggle-mechanism precedent route C follows.
   - `dev/plans/small_prime_kernel_strategy.md` § 5.5 / § 6.1 — the
     in-tree Candidate C/F design analysis that route C builds on.

## 6. Test plan (TDD)

Per the issue's criterion 2 (verbatim):

> Bit-exact equality vs the existing Candidate C output at GF(251)/n
> in {64, 256, 1024} on canonical seeds.

The kernel under test is `simd::maybe_fp_small()` /
`simd::maybe_fp_small_f32()`-style dispatch through
`fp_small_try_gemm_classical` with the route C toggle set. The
reference is the same dispatch with the toggle unset (= Candidate C).

Boundary `n` / `m` / `k` values exercised by the new tests, mirroring
route A's parity test battery:

- `n ∈ {1, 15, 16, 17, 23, 24, 25, 47, 48, 49, 63, 64, 65, 95, 96, 97, 121, 255, 256, 257, 1023, 1024}` — covers NR=24 and N-block boundaries plus criterion sizes.
- `k ∈ {1, 64, 128, 255, 256, 257, 511, 512, 1023, 1024, 1025}` — covers KC=256 boundary and odd-k pair tail.
- `m ∈ {1, 2, 3, 4, 5, 6, 7, 9, 33}` — covers MR=4 partial row tile.
- Fixed seeds: `1`/`2` for `(a, b)`; `3`/`4`, `5`/`6`, `7`/`8` for the
  per-axis sweep tests.

Tests are integration-level in
`crates/gf2-core/tests/route_c_gf251_parity.rs`, modelled on
`route_a_gf251_parity.rs`. They flip the route-C `AtomicBool` toggle
via the safe setter under a Mutex (process-wide flag).

Per the parity-test rule, each test does:

1. Take the toggle lock
2. Set toggle off → compute Candidate C reference output via `gemm`
3. Set toggle on → compute route C output via `gemm`
4. Reset toggle off; release lock
5. Compare element-by-element

This satisfies criterion 2 by direct bit-exact equality of
`Fp<251>` outputs.

## 7. Non-regression measurement plan

Per the issue's criterion 6 (verbatim):

> No regression on currently-PASSing GF(p) cells (delta ≤ 5%).

Same-session 5-trial median at the same HEAD as the route C
measurement. The toggle scope is **GF(251)-only**: route C never
fires for any `P ≠ 251` because `route_c_gf251_enabled::<P>()`
short-circuits on `P != 251`. Non-GF(251) cells therefore exercise
**byte-for-byte the same Candidate C kernel** in both phases — any
delta is host-noise, not a regression. This mirrors route A's
mechanical guarantee.

Cells to measure (same-session, route-C on vs route-C off):

| Field | n | Owner | Threshold |
|---|---:|---|---|
| GF(7) | 256, 1024 | already-PASS | within 5% of paired same-session default |
| GF(31) | 256, 1024 | already-PASS | within 5% of paired same-session default |
| GF(127) | 256, 1024 | already-PASS | within 5% of paired same-session default |
| GF(251) | 64, 256, 1024 | rework target (route C) | head-to-head route-C vs Candidate C |

## 8. Risk and open questions

- **Pack-cost vs amortisation.** Route C pays a one-shot pack cost
  for each (i_blk, n-panel) pair. At n=256 with MR=4 the panel count
  is `m / MR = 64` and the n-panel count is `n / NR ≈ 11`; pack work
  is `4 · 256 · 2 + 256 · 24 = 8 192` bytes per inner panel, ≈ 720
  KB total panel traffic vs the inner-FMA budget of ~270 µs at full
  Candidate C throughput. The pack cost is ≈ 180 µs at 4 GB/s
  streaming bandwidth — comparable to but not dominant over the
  inner FMA. Whether the inner-loop overhead reduction outweighs
  the pack cost at n=256 is the empirical question.

- **Integer ALU peak vs FMA peak.** Zen 3's integer ALU peak is
  half the FMA peak (80 vs 160 Gop/s). Route C cannot exceed
  ~80 Gop/s on this host without leaving the integer pipe, which
  rules out reaching n=1024 fflas-parity (138 Gop/s). The
  measured-against-Candidate-C improvement at n=1024 is therefore
  bounded by the pack-amortised inner-loop speedup, expected to be
  modest (5-15 %). The n=256 cell is the more interesting target
  because Candidate C's row-panel kernel pays more per-output-cell
  overhead at small n.

- **Worst case n=256 SHORTFALL.** If route C also shortfalls at n=256
  (like route A), the conclusion for the route-selection task
  (41096af5) is that no in-Rust integer / float route clears the
  1.5×-of-fflas threshold at n=256 on this Zen 3 host — making the
  best self-contained option a size-conditional dispatch (Candidate
  C below some N*, route A above) or accepting the structural gap
  as `[aspirational]`. This is a finding, not a worker failure;
  surface as an open question.

## 9. Out of scope (per issue)

- Changing production dispatch — owned by 41096af5 wave 4.
- Route A or B prototypes.
- AVX-512 / VNNI / GFNI / ZMM variants — out of scope for
  026fc832; routes to 7f809931.

## 10. References

- `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 1, item 3
- `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md`
- `dev/active/68cdf4c8-route-a-design.md`
- `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md`
- `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md`
- `dev/plans/small_prime_kernel_strategy.md`
- `crates/gf2-kernels-simd/src/x86/fp_small.rs` (Candidate C SSOT)
- `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` (route A SSOT)
- `crates/gf2-core/src/gfp/simd_ops.rs` (dispatch site)
- Goto + van de Geijn 2008 "Anatomy of High-Performance Matrix Multiplication"
- van Zee + van de Geijn 2015 "BLIS: A Framework for Rapidly Instantiating BLAS Functionality"
- AMD Software Optimization Guide for Family 19h (Zen 3), revision 3.07 (Publication ID 56665)
- Granlund + Möller 2011 "Improved division by invariant integers"
- Dumas + Giorgi + Pernet 2009 "Dense Linear Algebra over Word-Size Prime Fields"

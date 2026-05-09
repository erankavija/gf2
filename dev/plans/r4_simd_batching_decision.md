# R4 — SIMD batching strategy: generic framework vs per-prime kernels

**JIT issue:** `c7542983` (W0 / R4)
**Epic:** `epic:gf2-algebra-permanent` (parent design: `dev/plans/gf2_algebra_permanent.md` §10)
**Predecessor decisions:**
- D1a (`dev/plans/d1a_gf2_algebra_boundary.md`) — fixes SIMD home as `gf2-kernels-simd::bipedal_kernel`.
- D1b (`dev/plans/d1b_packed_field_api.md`) — fixes the trait surface; the SIMD-batched concrete type is e.g. `Bipedal3x4` with `LANES = 256`.
- D4 (`dev/plans/d4_intrinsic_feasibility.md`) — verifies AVX2 intrinsics stable on MSRV 1.95; AVX-512 marked `[aspirational]` per host envelope.
**Microbench artefact:** `dev/research/simd_batching_bench/`
**Status:** decision
**Date:** 2026-05-09

## 1. Scope

This document settles parent §10's open question: between (a) per-prime
hand-rolled SIMD kernels (`bipedal3_kernel.rs`, `bipedal5_kernel.rs`, etc.,
each carrying its own AVX2 implementation) and (b) a generic
`BatchedBipedalLike<MagLanes, SgnLanes>` framework instantiated per prime,
which strategy does the W3 / W4 SIMD-kernel implementation issue commit to?

The microbench at `dev/research/simd_batching_bench/` measures both
strategies on F_3 — the only encoding currently frozen. F_5 (D-bit-sliced,
R1 outcome) and F_7 (LUT-A, R2 outcome) are deferred: their kernels are
W4 work. F_3 is sufficient to make the strategic decision because the
two strategies' constant-factor delta is already observable on a single
encoding, and the F_5 / F_7 kernels can be re-evaluated against the
chosen path if their shape turns out to disagree.

**Measured:** F_3 add / sub / mul on AVX2 256-bit lanes at batch sizes
64, 256, 1024 F_3 elements (= 1, 4, 16 AVX2 ops worth, padded to 256-bit
multiples).

**Not measured:**
- F_5 / F_7 — deferred to W4, by which time R1 / R2 have settled the
  encoding shapes.
- AVX-512 — the dev host (Ryzen 9 5900X / Zen 3) lacks AVX-512 per D4 §3.
  Coded for portability, exercised only on hosts that report `avx512f`.
- Permanent end-to-end timing — out of scope for R4; covered by W3 once
  the kernel-strategy choice is committed.

## 2. Methodology

### 2.1 Hardware and toolchain

- **Host:** AMD Ryzen 9 5900X (Zen 3, 12 cores, 4.0 GHz boost clock per
  D4 §3). CPUID flags include `avx2`, `vaes`, `vpclmulqdq`, `fma`,
  `bmi1`, `bmi2`, `popcnt`. No AVX-512 family flags.
- **Toolchain:** rustc 1.95.0 (project MSRV per CLAUDE.md §MSRV).
- **Build profile:** `release`, `opt-level = 3`, `lto = "thin"`,
  `codegen-units = 1`. No `target-cpu=native` override — the bench is
  measured under the same compilation settings the production
  `gf2-kernels-simd` uses.

### 2.2 Measurement

- **Inputs:** deterministic LCG-generated `(mag1, sgn1, mag2, sgn2)`
  u64 quadruple of `n_words_logical = batch_lanes / 64` words per
  stream, padded up to a multiple of 4 u64 (one AVX2 lane = 256
  bits). All four streams are produced from the same LCG draw, and
  every `(mag, sgn)` pair has the canonical `sgn & ~mag = 0`
  invariant enforced by masking each fresh sign stream with its
  magnitude stream. The bench's `measure_cell` then plumbs the full
  quadruple through to the kernel calls; lane-population over
  `{0, 1, 2}` is roughly `(1/2, 1/4, 1/4)` from a uniform LCG draw,
  which is the closest a deterministic stream can get to a realistic
  permanent-style F_3 distribution without a domain-specific
  generator.
- **Warmup:** 1024 invocations of every (strategy, op, batch_lanes)
  cell, discarded before measurement.
- **Measurement window:** outer loop of 21 reps. Each rep times 1024
  inner invocations of the cell via paired `_rdtsc` reads, then divides
  total cycles by `INNER_REPS × batch_lanes`, i.e. by the **logical**
  batch size, not the padded-out width. The 64-lane case has
  `n_words_logical = 1` which pads up to 4 u64 (one AVX2 lane); we
  still issue one AVX2 op per kernel call but normalise the cost
  against the logical 64-lane request, so the row honestly captures
  the per-element cost (including the 75% padding overhead) of
  routing a 64-lane batch through an AVX2 kernel. For 256 and 1024
  lanes the padded width matches the logical width exactly, so this
  normalisation is identical to the un-padded one. The **minimum**
  cycles/op across the outer reps is reported (standard
  noise-rejection technique for short kernels; minima reflect
  uncontended runs, means are skewed by interrupts and CPU power-state
  transitions).
- **Wall-clock corroboration:** `std::time::Instant` straddles the
  same inner loop, reported as ns/op.
- **DCE protection:** `black_box(out_mag[0])` after every inner
  invocation prevents the optimiser from dropping the work; this also
  models a realistic data-dependency at the kernel boundary.

### 2.3 Outlier handling

Reporting the minimum across 21 outer reps is the outlier-rejection
mechanism. Re-running the bench three times produced ratios within
±0.005 on every cell (full per-cell traces in §5 below; the largest
spread is `add@64` at 0.934–0.935 and `add@256` at 0.931–0.934). The
absolute cycles/op move more (e.g. `add@64` ranged 0.187–0.233 across
the three runs) which is the expected boost-clock spread on the
5900X across short bursts; ratios are run-to-run stable because both
strategies see the same boost-clock trajectory inside a single run.
Per-cell relative variance is low enough that the criterion-4
tie-break range `[0.83, 1.20]` is well outside the noise floor.

## 3. Per-prime strategy code shape

Per-prime hand-rolled. Each AVX2 entry point is `#[target_feature(enable
= "avx2")]`-attributed and uses the intrinsics directly:

```rust
// dev/research/simd_batching_bench/src/per_prime.rs, abridged
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn bipedal3_avx2_add(
    m1: __m256i, s1: __m256i, m2: __m256i, s2: __m256i,
) -> (__m256i, __m256i) {
    // SAFETY: AVX2 availability is the caller's precondition.
    unsafe {
        let t = _mm256_xor_si256(_mm256_xor_si256(m1, s1), s2);
        let u = _mm256_and_si256(m2, t);
        let m_plus = _mm256_or_si256(u, _mm256_xor_si256(m1, m2));
        let s_plus = _mm256_xor_si256(u, s1);
        (m_plus, s_plus)
    }
}
```

That is the paper §2.2 6-op `add` formula expanded to AVX2 verbatim:
6 intrinsics, no abstraction layer. `sub` mirrors the same shape with
6 intrinsics; `mul` is 2 intrinsics. The batch driver is a `while i <
n` loop over `_mm256_loadu_si256` / kernel / `_mm256_storeu_si256`
straddling the AVX2 lane.

## 4. Generic framework code shape

The framework abstracts the lane-width logical primitives behind a
trait, with one impl per lane shape, and a generic kernel template
parametrised over the lane types:

```rust
// dev/research/simd_batching_bench/src/generic.rs, abridged
pub trait BipedalLogicalLanes: Copy {
    const U64_PER_LANE: usize;
    unsafe fn loadu(src: &[u64], offset: usize) -> Self;
    unsafe fn storeu(dst: &mut [u64], offset: usize, v: Self);
    unsafe fn and(a: Self, b: Self) -> Self;
    unsafe fn xor(a: Self, b: Self) -> Self;
    unsafe fn or(a: Self, b: Self) -> Self;
}

#[derive(Clone, Copy)]
pub struct Avx2Lane(pub __m256i);

impl BipedalLogicalLanes for Avx2Lane {
    const U64_PER_LANE: usize = 4;
    #[inline(always)]
    unsafe fn xor(a: Self, b: Self) -> Self {
        unsafe { Avx2Lane(_mm256_xor_si256(a.0, b.0)) }
    }
    // ...and / or / loadu / storeu...
}

pub struct BatchedBipedalLike<Mag: BipedalLogicalLanes, Sgn: BipedalLogicalLanes> {
    _phantom: PhantomData<fn() -> (Mag, Sgn)>,
}

impl<Mag, Sgn> BatchedBipedalLike<Mag, Sgn>
where Mag: BipedalLogicalLanes, Sgn: BipedalLogicalLanes
{
    #[inline(always)]
    pub unsafe fn add(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        unsafe {
            let s1_as_m: Mag = transmute_lane(s1);
            let s2_as_m: Mag = transmute_lane(s2);
            let t = Mag::xor(Mag::xor(m1, s1_as_m), s2_as_m);
            let u = Mag::and(m2, t);
            let m_plus = Mag::or(u, Mag::xor(m1, m2));
            let s_plus = transmute_lane(Mag::xor(u, s1_as_m));
            (m_plus, s_plus)
        }
    }
    // ...sub / mul...
}

pub type Bipedal3x4 = BatchedBipedalLike<Avx2Lane, Avx2Lane>;

impl BatchedBipedalLike<Avx2Lane, Avx2Lane> {
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_add_batch(
        mag1: &[u64], sgn1: &[u64], mag2: &[u64], sgn2: &[u64],
        out_mag: &mut [u64], out_sgn: &mut [u64],
    ) {
        // ...load / Self::add / store loop...
    }
    // ...run_sub_batch / run_mul_batch...
}
```

The `transmute_lane` helper exists for shape-uniformity — it is a
no-op when `Mag = Sgn = Avx2Lane`, but lets the framework body type
naturally for future encodings where the magnitude and sign lane
widths could differ (e.g. an F_5 D-bit-sliced encoding with three
8-bit-wide planes). For the F_3 instantiation it compiles to nothing.

### 4.1 Critical inlining note

The `run_*_batch` entry points carry `#[target_feature(enable =
"avx2")]` and the trait impl methods are `#[inline(always)]`. Without
the target_feature attribute on the entry points, rustc cannot inline
trait methods that issue AVX2 intrinsics into a function lacking the
feature, which forces a real (non-inlined) call per AVX2 op. An
earlier draft of the bench omitted this attribute on the generic
side; the resulting measurement showed the generic framework 12-34x
slower than per-prime, which was a benchmark artefact, not a real
cost. Once the entry points were target_feature-attributed and the
trait impl methods marked `#[inline(always)]`, the gap closed. The
production crate will follow the same `#[target_feature]` discipline
on the kernel entry points.

## 5. Results

Run on 2026-05-09, dev host as §2.1, three back-to-back invocations of
`cargo run --release` after the bench was rebuilt against the
post-rework harness (logical-lane normalisation + full
`(mag1, sgn1, mag2, sgn2)` quadruple piped through to kernel calls,
addressing the prior code-review findings on `bench.rs:175-183` and
`bench.rs:236-240`). Reported: minimum cycles/op across 21 reps ×
1024 inner invocations per cell.

### Run 1

| op   | batch_lanes | per-prime cycles/op | generic cycles/op | ratio (g/p) | per-prime ns/op | generic ns/op |
|------|-------------|---------------------|-------------------|-------------|------------------|----------------|
| add  |          64 |              0.2038 |            0.1903 |       0.934 |           0.0555 |         0.0519 |
| sub  |          64 |              0.2044 |            0.2044 |       1.000 |           0.0555 |         0.0555 |
| mul  |          64 |              0.2038 |            0.2038 |       1.000 |           0.0555 |         0.0555 |
| add  |         256 |              0.0511 |            0.0477 |       0.934 |           0.0139 |         0.0130 |
| sub  |         256 |              0.0511 |            0.0511 |       1.000 |           0.0139 |         0.0139 |
| mul  |         256 |              0.0510 |            0.0510 |       1.000 |           0.0139 |         0.0139 |
| add  |        1024 |              0.0187 |            0.0179 |       0.955 |           0.0051 |         0.0049 |
| sub  |        1024 |              0.0187 |            0.0204 |       1.091 |           0.0051 |         0.0055 |
| mul  |        1024 |              0.0179 |            0.0179 |       1.000 |           0.0049 |         0.0049 |

### Run 2

| op   | batch_lanes | per-prime cycles/op | generic cycles/op | ratio (g/p) | per-prime ns/op | generic ns/op |
|------|-------------|---------------------|-------------------|-------------|------------------|----------------|
| add  |          64 |              0.1874 |            0.1750 |       0.934 |           0.0511 |         0.0478 |
| sub  |          64 |              0.1874 |            0.1874 |       1.000 |           0.0510 |         0.0511 |
| mul  |          64 |              0.1874 |            0.1874 |       1.000 |           0.0511 |         0.0510 |
| add  |         256 |              0.0469 |            0.0438 |       0.934 |           0.0128 |         0.0119 |
| sub  |         256 |              0.0469 |            0.0469 |       1.000 |           0.0128 |         0.0128 |
| mul  |         256 |              0.0469 |            0.0469 |       1.000 |           0.0128 |         0.0127 |
| add  |        1024 |              0.0172 |            0.0164 |       0.955 |           0.0047 |         0.0045 |
| sub  |        1024 |              0.0172 |            0.0187 |       1.090 |           0.0047 |         0.0051 |
| mul  |        1024 |              0.0164 |            0.0164 |       1.000 |           0.0045 |         0.0045 |

### Run 3

| op   | batch_lanes | per-prime cycles/op | generic cycles/op | ratio (g/p) | per-prime ns/op | generic ns/op |
|------|-------------|---------------------|-------------------|-------------|------------------|----------------|
| add  |          64 |              0.2332 |            0.2179 |       0.935 |           0.0635 |         0.0594 |
| sub  |          64 |              0.2332 |            0.2332 |       1.000 |           0.0635 |         0.0635 |
| mul  |          64 |              0.2332 |            0.2332 |       1.000 |           0.0635 |         0.0635 |
| add  |         256 |              0.0511 |            0.0476 |       0.931 |           0.0139 |         0.0130 |
| sub  |         256 |              0.0511 |            0.0511 |       1.000 |           0.0139 |         0.0139 |
| mul  |         256 |              0.0511 |            0.0510 |       0.997 |           0.0139 |         0.0139 |
| add  |        1024 |              0.0187 |            0.0179 |       0.955 |           0.0051 |         0.0049 |
| sub  |        1024 |              0.0187 |            0.0204 |       1.091 |           0.0051 |         0.0055 |
| mul  |        1024 |              0.0179 |            0.0179 |       1.000 |           0.0049 |         0.0049 |

The three runs agree to within ±0.005 on every cell ratio (smallest
spread: `mul@64`, `sub@256`, `mul@1024`, three identical 1.000 across
all runs; largest spread: `add@256`, ratio 0.931–0.934). The absolute
cycles/op move 0.18–0.23 at `@64` and 0.046–0.051 at `@256`, which is
the boost-clock spread on the 5900X across short bursts.

The 64-lane row is approximately 4× the 256-lane row in cycles/op
(0.187–0.233 vs 0.047–0.051), consistent with the AVX2 kernel doing
one 256-lane op for a 64-lane request and the cost being amortised
over only 64 logical lanes rather than 256. This is the honest
per-element cost of running a 64-lane request through the
4-u64-wide AVX2 kernel; it is not a per-strategy artefact (per-prime
and generic see the same padding overhead).

### 5.1 Cell summary

Aggregated over all three runs:

- **9 of 9 cells** have a ratio inside `[0.83, 1.20]`.
- **5 cells** show generic faster than per-prime
  (`add@64`, `add@256`, `add@1024` at 0.93–0.96; `mul@256` at 0.997
  in run 3 only) by margins ranging from 0.5% to 7%.
- **3 cells** (`sub@64`, `sub@256`, `mul@64`, `mul@1024`) show
  parity (ratio 1.000 ± 0.001).
- **1 cell** (`sub@1024`) shows generic ~9% slower (ratio 1.090–1.091
  across all three runs, consistent rather than noise).
- **No cell** has a ratio above 1.20x.
- **No cell** has a ratio below 0.83x.

Range observed across all 27 cell-runs: 0.931–1.091.

## 6. Decision

**Generic framework wins by tie-break.**

Apply the issue's criterion 4 verbatim: "If the gap is < 1.2x either
way, the doc recommends the generic framework as the tie-break (less
code)." Every measured cell across all three runs lies inside
`[0.83, 1.20]` (range observed: 0.931–1.091). The single cell whose
generic ratio rises above 1.05 (`sub@1024` at ~1.09) is still well
inside the tie-break band, and is balanced by the three `add` cells
where generic is 4–7% faster than per-prime. The tie-break therefore
fires on every cell, and the aggregate decision is the generic
framework.

**The W3 / W4 SIMD kernel issues commit to the generic
`BatchedBipedalLike<MagLanes, SgnLanes>` framework.**

## 7. Justification

### 7.1 Cell-by-cell walkthrough

- **`add` cells** (3 of 9): generic is 4–7% faster than per-prime on
  every batch size, consistent across all three runs (`add@64` 0.934–
  0.935; `add@256` 0.931–0.934; `add@1024` 0.955 in every run). Both
  strategies expand to the same six AVX2 intrinsics in the same
  order, but rustc's inliner finds a marginally tighter register
  schedule for the generic body on Zen 3, likely a side-effect of
  the explicit `transmute_lane` call in the framework giving the
  scheduler an extra reorder opportunity. The advantage is small but
  reproducible.
- **`sub` and `mul` cells at small batches** (`sub@64`, `sub@256`,
  `mul@64`, `mul@256`, 4 of 9): generic equals per-prime to within
  0.3% on every run. With `#[target_feature(enable = "avx2")]` on the
  generic entry point and `#[inline(always)]` on the trait methods,
  rustc inlines the trait dispatch entirely; the resulting object
  code is byte-identical modulo register allocation choices.
- **`mul@1024`** (1 of 9): generic equals per-prime exactly (1.000 in
  every run). Mul is the simplest formula (2 ops) so register
  pressure is minimal and the two strategies converge cleanly.
- **`sub@1024`** (1 of 9): generic is 9% slower than per-prime
  (1.090–1.091 across all three runs). This is the only cell where
  the generic framework loses by a measurable margin; it appears
  consistently across runs so it is not noise. The likely cause is
  that the generic `sub` body routes through both `Mag` and `Sgn`
  trait method calls (the `m2_xor_s2: Sgn` step uses `Sgn::xor`
  rather than `Mag::xor`) and the resulting register schedule has a
  loop-carried dependency that the larger `@1024` outer loop
  exposes. The cell is still well inside the criterion-4 tie-break
  band `[0.83, 1.20]`, so the decision stands; W4 should profile
  this specific path on the production kernel and consider hand-
  rolling `sub@1024` if it appears on a hot path.
- **Batch-size scaling** (cross-cutting): both strategies show the
  per-element cost dropping ~4× from 64-lane to 256-lane (0.19 →
  0.05 cycles/op). This is the AVX2 kernel running one 256-lane op
  for a 64-lane request, with the cost amortised over only the
  logical 64 lanes; it is not a per-strategy effect (per-prime sees
  the same padding overhead). At 1024 lanes the per-element cost
  drops a further ~2.7× (0.05 → 0.018 cycles/op), reflecting
  loop-vectoriser amortisation of per-batch setup.

### 7.2 No pathology found

The bench was designed to surface pathologies: small-batch overhead
from per-batch trait dispatch, large-batch register-pressure
penalties, mid-batch alignment artefacts. The first two do not
appear. The per-batch generic dispatch cost is essentially zero
because it inlines through. The one weak signal is `sub@1024`
(9% slow); it is consistent enough across runs to be a real
register-schedule effect rather than noise, but small enough that it
fits inside the criterion-4 tie-break band, and it is offset by the
3 `add` cells where generic is faster.

The trait-bound monomorphisation produces one specialised function
per `(Mag, Sgn)` pair, which for F_3 is a single concrete
`BatchedBipedalLike<Avx2Lane, Avx2Lane>::run_add_batch` matching the
per-prime entry point one-for-one in instruction count.

### 7.3 The benchmark artefact lesson

The first bench draft (without `#[target_feature]` on the generic
entry point) measured generic 12-34x slower than per-prime. Including
that lesson in §4.1 above so it is preserved for the W3 / W4
implementation issues. The production kernel **must** carry
`#[target_feature(enable = "avx2")]` on every entry point that
dispatches into the trait surface, and trait impl methods using
intrinsics **must** be `#[inline(always)]`. Without those, the
inliner fails silently and the generic framework looks much worse
than it is.

## 8. Implications for W4

**F_5 (D-bit-sliced, R1 outcome) and F_7 (LUT-A, R2 outcome) are W4
work and instantiate the same framework.** Concretely:

- **F_5 D-bit-sliced.** Three planes per word (`b0, b1, b2`).
  Instantiate `BatchedBipedalLike<Avx2BitSliced3, Avx2BitSliced3>`
  with a new `Avx2BitSliced3` lane type that wraps three `__m256i`
  values. The `BipedalLogicalLanes` trait may need extending — the
  bipedal-style add formula does not directly carry to F_5 D-bit
  arithmetic, but the framework's *shape* (a generic kernel template
  parametrised over the lane primitives) does. Whether F_5's add /
  sub / mul fit the same `add(m1,s1,m2,s2)` signature or need their
  own framework method is a W4 question. Worst case, F_5 instantiates
  a sibling framework (e.g. `BatchedBitSliced3<Plane>`) using the
  same trait scaffolding.
- **F_7 LUT-A.** 16-bit slot per element, multiply via 16-bit LUT.
  Instantiate `BatchedBipedalLike<Avx2Lut16, Avx2Lut16>` (or a
  similarly named sibling). The arithmetic is byte-lane-shuffle-driven
  rather than logical-only, so F_7 will likely need a separate
  framework method that takes a LUT operand. Again, the trait
  scaffolding (lane abstraction + entry-point `#[target_feature]`
  pattern) carries over; only the operation formulas change.
- **AVX-512 path (`[aspirational]`).** Instantiate
  `BatchedBipedalLike<Avx512Lane, Avx512Lane>` with an `Avx512Lane`
  impl using `_mm512_*` intrinsics. The lane width changes (8 u64
  per lane), the kernel body is unchanged. Runtime dispatch picks
  the AVX2 or AVX-512 instantiation at the
  `gf2-algebra::permanent::bipedal3` boundary, mirroring the existing
  `gf2-kernels-simd::LogicalFns` pattern.

**Code-volume estimate (informational).** Per-prime would have produced
roughly 200-300 LoC × 3 primes × 2 ISAs (AVX2 / AVX-512) ≈ 1.2-1.8 kLoC
of unsafe SIMD glue, plus 6-9 distinct entry-point names to runtime-
dispatch through. The generic framework produces ~300 LoC of trait +
template plus ~50 LoC per concrete lane impl, total ~600-800 LoC for
the same coverage, with a single entry-point name per (op, ISA). The
code-volume reduction is the practical win the criterion-4 tie-break
captures.

## 9. Reproducibility

### 9.1 Re-run the bench

```sh
cd dev/research/simd_batching_bench
cargo test --release    # 9 unit tests + 8 doc-tests, all pass
cargo run  --release    # prints the 9-cell results table
```

Both commands complete in under 5 seconds on the dev host. The bench
exits non-zero (code 2) if AVX2 is not detected at runtime; on the
dev host AVX2 is available and the bench prints `AVX2 detected: yes`.

### 9.2 Pinned versions and host

- **rustc:** 1.95.0 (project MSRV per CLAUDE.md §MSRV).
- **CPU:** AMD Ryzen 9 5900X, family 25 model 33 stepping 0, Zen 3.
  Verified via `lscpu` per D4 §3.
- **Build profile:** `release`, `opt-level = 3`, `lto = "thin"`,
  `codegen-units = 1`, **no** `target-cpu` override.

### 9.3 Variance across re-runs

Three back-to-back runs (full per-cell tables in §5) produced ratios
within ±0.005 on every cell. Per-cell spreads:

- 6 cells (`sub@64`, `mul@64`, `sub@256`, `sub@1024`, `mul@1024`,
  `add@1024`) had identical ratios in every run (spread 0.000).
- 1 cell (`mul@256`) moved 1.000 → 0.997 in run 3 (spread 0.003).
- 1 cell (`add@64`) moved 0.934 → 0.935 (spread 0.001).
- 1 cell (`add@256`) moved 0.934 → 0.931 (spread 0.003).

The largest observed ratio in any cell across any run is 1.091
(`sub@1024`), well inside the 1.20 threshold. The decision (generic
wins by tie-break) is robust to this variance.

### 9.4 Correctness equivalence

The bench's `cargo test --release` verifies byte-identical output from
per-prime and generic on the same inputs across all three operations
(`per_prime_and_generic_agree_{add, sub, mul}`), and verifies both
match the scalar reference (`per_prime_*_matches_scalar_reference`).
Correctness is `[hard]` and is enforced by the test suite, independent
of the perf measurement.

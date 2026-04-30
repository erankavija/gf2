# gf2-algebra: fast matrix permanents over F₃ / F₅ / F₇

**Epic slug:** `epic:gf2-algebra-permanent`
**Status:** design / W0
**Reference paper:** Danny Scheinerman, *Fast computation of permanents over F₃ via F₂ arithmetic*, arxiv 2407.20205v2 (Aug 2024).

## 1. Motivation

The reference paper introduces a "bipedal" map encoding F₃ⁿ as a pair of F₂ⁿ vectors and uses bitwise primitives to implement add/sub/mul/div in 1–6 ops each. Combined with Ryser's permanent formula in Gray-code order, the resulting `permanent_mod3` routine achieves a reported 86.9× speedup over a naive Ryser implementation in Julia, single-threaded, on a 4.20 GHz desktop. The paper does not exploit SIMD, multi-thread parallelism, or GPU.

This epic sets out to:

1. Reproduce the paper's headline results in Rust (in-tree, deterministic).
2. **Beat** them by ≥ 50× single-thread vs the in-tree Rust port, with linear parallel scaling on multicore, and demonstrate competitive GPU performance at large *n*.
3. Generalise the bipedal idea to F₅ and F₇ via best-effort encoding research.
4. Mechanically verify (Lean4 via Charon/Aeneas) the bipedal F₃ arithmetic and Ryser's formula on the project's `FiniteField` trait.
5. Produce a publication-grade benchmark artefact (criterion + plots + reproducible scripts).

The technique fits the project's research-grade-toolkit charter (CLAUDE.md §Vision). All required infrastructure — `Vec<u64>`-backed bit storage with `mask_tail`, AVX2/AVX-512 logical kernels, a rayon CPU backend, an HIP/ROCm GPU crate, and a Charon/Aeneas Lean pipeline — already exists in the workspace.

## 2. Reference paper, in detail

### 2.1 Bipedal representation

Define φ: F₃ → F₂ × F₂ by

```
φ(0)  = (0, 0)
φ(1)  = (1, 0)
φ(-1) = (1, 1)
```

extended pointwise to vectors. Each F₃ⁿ value is represented as a pair `(mag, sgn) ∈ (F₂ⁿ)²`. The decoder ψ: F₂ × F₂ → F₃ also maps `(0, 1) ↦ 0` (alternative-zero), giving a 4-state encoding with one redundant codeword. This redundancy is what permits the cheap arithmetic.

### 2.2 Bipedal arithmetic (Theorem 2.1, paper)

For pairs `x = (m₁, s₁)`, `y = (m₂, s₂)`:

- **Multiplication** (2 ops):
  ```
  m× = m₁ & m₂
  s× = s₁ ^ s₂
  ```
- **Addition** (6 ops with CSE):
  ```
  t  = m₁ ^ s₁ ^ s₂
  u  = m₂ & t
  m+ = u | (m₁ ^ m₂)
  s+ = u ^ s₁
  ```
- **Subtraction** (6 ops):
  ```
  t  = s₁ ^ s₂
  u  = m₁ & t
  m- = u | (m₁ ^ m₂)
  s- = u ^ (m₂ ^ s₂)
  ```
- **Division by nonzero** (1 op): `m÷ = m₁`, `s÷ = s₁ ^ s₂`.

These operations are lane-parallel: a single 64-bit AND/XOR processes 64 independent F₃ elements. `gf2-kernels-simd::LogicalFns` exposes vectorised AND/XOR/OR/POPCNT over `&mut [u64]`, so a packed `Bipedal3Vec` of any length composes directly without writing a new kernel for the basic ops.

### 2.3 Ryser's formula

```
perm(A) = (-1)ⁿ · Σ_{S ⊆ {1..n}} (-1)^{|S|} · ∏ᵢ (Σ_{j ∈ S} a_{ij})
```

Naive enumeration is O(n²·2ⁿ). Iterating subsets in Gray-code order — flipping exactly one bit per step — reduces the column-sum update to a single vector add/sub, giving O(n·2ⁿ) field operations. With bipedal packing, one F₃ vector add or subtract is O(n/64) word ops. For n ≤ 64 this is one word op, so the inner loop is **effectively O(2ⁿ) primitive `u64` operations**.

### 2.4 Paper Table 2 (verbatim)

Single-thread Julia, 4.20 GHz desktop, time in seconds:

| n  | permanent_Ryser | permanent_mod3 |
|----|-----------------|----------------|
| 24 |          1.96   |          0.025 |
| 26 |          7.98   |          0.099 |
| 28 |         32.3    |          0.401 |
| 30 |        131.5    |          1.59  |
| 32 |        533.5    |          6.34  |
| 34 |       2166.9    |         25.52  |
| 36 |       8857.9    |        101.9   |

Headline ratio at n=36: **86.9×**. Parallel demo: n=50 in 832 CPU-hours on 128 cores (a different, large-scale benchmark not in Table 2).

### 2.5 Secondary claim

Monte Carlo evidence in the paper suggests perm(A) tends to the uniform distribution on F₃ as n → ∞ for random {-1,0,1} matrices. This is a `type:simulation` reproduction target.

## 3. Scope and non-goals

### In scope

- Bipedal F₃ packed types (element, vector, matrix) with paper's bitwise formulas.
- Generic `permanent_ryser<F: FiniteField>` reference implementation.
- Specialised `permanent_bipedal3` single- and multi-word fast paths.
- AVX2 SIMD acceleration on Zen 3 (the dev host); AVX-512 path coded but `[aspirational]`-rated since the dev host lacks AVX-512.
- Rayon parallel permanent with chunk-size sweep.
- F₅ and F₇ packed types via best-effort encoding research; permanents over those fields.
- HIP/ROCm GPU permanent kernels (gfx1030).
- Lean4 mechanical proofs of (a) bipedal F₃ arithmetic correctness vs `Fp<3>`, (b) Ryser's formula on `FiniteField`, scoped to the bounded n ≤ 63 single-`u64` regime.
- Publication-grade benchmark artefact (criterion + plots + reproducible scripts).
- Monte Carlo distribution-of-perm verification.

### Non-goals

- No general-purpose char-3 linear algebra (det, rank, char-poly, Smith normal form). These are natural follow-ups but out of scope for this epic.
- No char-2 permanents (over F₂, perm = det, already trivial via existing rank/RREF code).
- No primes p ≥ 11 beyond what falls out for free.
- No proof of unbounded-n Ryser (would require formalising arbitrary-arity sums).
- No reproduction of the paper's n=50 / 832-CPU-hour run on 128 cores — we'll do a smaller analogous run that demonstrates the speedup factor.

## 4. Hardware envelope

Dev host (verified 2026-05-01): AMD Ryzen 9 5900X (Zen 3, 12-core / 24-thread). Flags include AVX2, FMA, AES-NI, VAES, VPCLMULQDQ, SSE4.2, BMI1/BMI2, **but no AVX-512**.

Implications:
- Headline single-thread number: AVX2-based bipedal kernel, 256-bit lanes (4 × u64).
- AVX-512 paths must be coded (for portability) but rated `[aspirational]`. CI runs only the AVX2 path on this host.
- 12 physical cores ⇒ S2 parallel-scaling target is 0.85×·12 ≈ 10× over single-thread ST. With SMT, a stretch goal is ≥ 12×.
- GPU host: existing gf2-kernels-hip target gfx1030 (RX 6000 series). Same host, opt-in `--features hip`.

The 4.20 GHz Julia desktop in the paper is unspecified. **Direct cross-language cross-machine comparison is unreliable**; the [hard] perf criterion targets in-tree Rust-vs-Rust (T8 baseline). Vs Julia is `[aspirational]`.

## 5. Crate architecture: `gf2-algebra`

New top-level workspace member `crates/gf2-algebra/` with these modules:

```
gf2-algebra
├── Cargo.toml          # MSRV 1.95; default-features = ["simd","parallel"]
├── src/
│   ├── lib.rs          # #![deny(unsafe_code)]; re-exports
│   ├── packed/
│   │   ├── mod.rs      # PackedField<F> trait
│   │   ├── bipedal3.rs # Bipedal3 element + Vec + Matrix
│   │   ├── packed5.rs  # F_5 packed (post-R1)
│   │   └── packed7.rs  # F_7 packed (post-R2)
│   ├── permanent/
│   │   ├── mod.rs      # Permanent trait + dispatch
│   │   ├── ryser.rs    # generic permanent_ryser<F>
│   │   ├── bipedal3.rs # permanent_bipedal3 (single + multi-word)
│   │   ├── bipedal5.rs # permanent_bipedal5
│   │   ├── bipedal7.rs # permanent_bipedal7
│   │   └── reference.rs# permanent_mod3_reference (faithful Julia port)
│   ├── gray.rs         # gray_code_iter — shared subset enumerator
│   ├── parallel.rs     # rayon dispatcher (#[cfg(feature="parallel")])
│   └── gpu.rs          # HIP host-side glue (#[cfg(feature="hip")])
├── benches/
│   ├── permanent.rs    # criterion suite
│   └── packing.rs      # bipedal vs Fp<P> microbenches
├── examples/
│   ├── permanent_demo.rs
│   └── reproduce_table2.rs
└── tests/
    ├── bipedal3_axioms.rs
    ├── permanent_cross_check.rs
    └── multi_word_streaming.rs
```

Workspace dependency graph additions:
- `gf2-algebra` → `gf2-core` (for `FiniteField`, `BitVec`, `Fp<P>`)
- `gf2-algebra` → `gf2-kernels-simd` (feature-gated `simd`)
- `gf2-algebra` → `gf2-kernels-hip` (feature-gated `hip`; will not be in default workspace)

`#![deny(unsafe_code)]` everywhere. SIMD code (`unsafe`) lives in `gf2-kernels-simd::bipedal_kernel` (added by T12). GPU code (`unsafe`) lives in `gf2-kernels-hip::permanent` (added by T22).

## 6. The `PackedField<F>` trait

**Critical design point.** A `Bipedal3` value packs many F₃ elements into two `u64`s; it is *not* a single field element and **must not** implement `FiniteField`. Instead `gf2-algebra::packed::PackedField<F>` is a new trait abstracting "lane-parallel arithmetic over an underlying scalar field":

```rust
pub trait PackedField<F: FiniteField>: Copy + Eq + std::fmt::Debug {
    /// Number of independent F-lanes packed into one Self.
    const LANES: usize;

    /// All-lanes-zero / all-lanes-one constants in the underlying field.
    fn zero() -> Self;
    fn one() -> Self;

    /// Lane-wise arithmetic.
    fn add(self, rhs: Self) -> Self;
    fn sub(self, rhs: Self) -> Self;
    fn neg(self) -> Self;
    fn mul(self, rhs: Self) -> Self;

    /// Broadcast a scalar to all lanes.
    fn splat(x: F) -> Self;

    /// Extract / set a single lane.
    fn lane(self, i: usize) -> F;
    fn with_lane(self, i: usize, x: F) -> Self;

    /// All-lanes-equal predicate (used to short-circuit the Ryser inner product).
    fn all_zero(self) -> bool;
}
```

`Bipedal3 : PackedField<Fp<3>>` with `LANES = 64`. There is *also* a separate `PackedFieldVec<F>` interface for variable-length packed vectors (a `Vec<u64>` pair under the hood) — that is what `Bipedal3Vec` and `Bipedal3Matrix` build on. The exact type signatures are settled in W0 issue **D1b** before W1 begins; this section is the strawman input.

The `Permanent` trait sits next to `PackedField`:

```rust
pub trait Permanent<F: FiniteField> {
    type Matrix;
    fn permanent(&self) -> F;
}
```

Implemented for:
- generic dense matrices via `permanent_ryser<F>` (T7)
- `Bipedal3Matrix` via `permanent_bipedal3` (T9, T14)
- `Bipedal5Matrix`, `Bipedal7Matrix` (T18, T20)

Dispatch (SIMD, parallel, GPU) is internal to each `permanent_*` impl, gated by features.

## 7. F₃ bipedal — exact code shape

### 7.1 Element type

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Bipedal3 { mag: u64, sgn: u64 }

impl Bipedal3 {
    pub const ZERO: Self = Self { mag: 0, sgn: 0 };
    pub const ONE:  Self = Self { mag: !0, sgn: 0 };

    #[inline] pub fn add(self, r: Self) -> Self {
        let t = self.mag ^ self.sgn ^ r.sgn;
        let u = r.mag & t;
        Self { mag: u | (self.mag ^ r.mag), sgn: u ^ self.sgn }
    }
    #[inline] pub fn mul(self, r: Self) -> Self {
        Self { mag: self.mag & r.mag, sgn: self.sgn ^ r.sgn }
    }
    // …sub, div, neg
}
```

T3 builds this with property tests (`proptest`, 10k random pairs, all ops match `Fp<3>`) and exhaustive lane-truth-table tests for n ≤ 16.

### 7.2 Vec and Matrix

`Bipedal3Vec` = pair of `Vec<u64>` with shared length, mask_tail invariant per CLAUDE.md §Key design invariants. Word-boundary tests at lengths 0, 1, 63, 64, 65, 127, 128, 129.

`Bipedal3Matrix` is row-major `Bipedal3Vec` with stride for SIMD alignment. `permanent_bipedal3` consumes a `Bipedal3Matrix` and produces an `Fp<3>` value.

### 7.3 The Gray-code Ryser inner loop (paper algorithm)

```rust
pub fn permanent_bipedal3_single(mat: &Bipedal3Matrix) -> Fp<3> {
    debug_assert!(mat.n() <= 64);
    let n = mat.n();
    let mut col_sum = Bipedal3Vec::zero(n);  // fits in one (mag, sgn) pair
    let mut acc = Fp3Accumulator::zero();
    for (k, flip) in gray_code_iter(n).enumerate() {
        if added(k, flip) {
            col_sum.add_in_place(mat.column(flip));
        } else {
            col_sum.sub_in_place(mat.column(flip));
        }
        let prod = col_sum.fold_mul();           // packed product over n lanes
        if prod.is_zero() { continue; }
        let parity = (k.count_ones() & 1) as u64;
        acc.add_signed(prod.to_fp3(), parity);
    }
    if n & 1 == 1 { acc.negate(); }
    acc.value()
}
```

The `fold_mul` reduction is log-tree: `mul` has 2 ops per step, total `2·log₂(n)` u64 ops. For n=36 that's ~12 ops total per Gray step.

The Gray-code iterator (T6) is shared with generic `permanent_ryser<F>` — no duplication.

## 8. F₅ / F₇ encoding — research bounds (R1, R2)

**Thesis to test.** F₅ and F₇ admit "bipedal-like" multi-bit encodings minimising the bitwise op count for add/mul. The simplest baseline is 3-bit-per-element packed (16 elements per `u64`) with a 2¹⁶-entry table-LUT for mul. Better encodings may exist by exploiting the multiplicative structure (F₅* is cyclic of order 4; F₇* of order 6) — e.g., a (log, sign) split for F₅, or a specialised redundant 4-bit code for F₇.

R1 and R2 are `type:simulation` issues that prototype candidate encodings in `dev/research/{f5,f7}_packing/` (mirroring `dev/research/rns_prototype/`), measure cycles/op, and produce a decision document selecting the winning candidate. **User approval required** before R1/R2 close — these are best-effort research items where the encoding choice has wide impact on the W4 implementation.

**Hard fallback** if R1/R2 don't yield novel encodings beating the LUT baseline: 3-bit-per-element packed + 2¹⁶ LUT mul. This still gives a useful speedup on top of `Fp<5>`/`Fp<7>` Montgomery arithmetic and lets W5/W7 proceed without blocking.

## 9. Multi-word streaming column-sum (n > 64)

For n > 64 the column-sum vector spans multiple `u64`-pairs. The Ryser inner loop becomes:

```
column_sum[w] = column_sum[w] ± A[w][flip]   for w in 0..ceil(n/64)
prod = fold_mul over all words
```

`A[w][flip]` is the `flip`-th column's `w`-th word — i.e., we walk down a column. To make this cache-friendly we store the matrix in **column-major** order for the multi-word path: `A.col(j)` is contiguous. (Single-word path is column-major too; this is a uniform layout.) Cache-block by L1 footprint so the column-sum vector and the touched column both fit. R3 produces the cache-blocking design.

For n in the 65–256 range, multi-word streaming is correct against a naive multi-precision reference (T14's success criterion). Beyond 256 we fall back to the parallel/GPU path because single-thread time grows like 2ⁿ regardless.

## 10. SIMD batching strategy

Two strategies, picked by R4 microbench:

- **Per-prime hand-rolled.** A `bipedal3_kernel.rs` writes specialised AVX2/AVX-512 functions for `Bipedal3` vectors; F₅ and F₇ each get their own kernel files. Maximum performance per prime, more code.

- **Generic `BatchedBipedalLike<P, MagLanes, SgnLanes>` framework.** A single SIMD primitive parametrised over the encoding shape; F₃, F₅, F₇ instantiate the same kernel template. Less code, may sacrifice constant factor.

R4 measures both and picks one. The decision is logged in `dev/plans/r4_simd_batching_decision.md` and attached to the R4 issue. Either way, T12 (W3) commits to the choice for F₃ first, then T21 (W4) extends to F₅/F₇.

## 11. GPU strategy

`gf2-kernels-hip/hip/permanent_*.hip` device kernels, opt-in via `--features hip`. Each kernel takes a flat `(mag[], sgn[])` representation of the column-sum buffer and a chunk of Gray-code work indices, and accumulates the partial signed product sum into a per-thread accumulator. Host-side reduction in `gf2-algebra::gpu`.

The work decomposition mirrors the rayon design (block-of-Gray-steps per work unit), so once W3 settles the chunking strategy, W5 reuses the same partition logic. **W5 is gated on W3 + S2 closing** for that reason.

GPU performance target (S5): GPU wins over CPU SIMD at n ≥ 40 for F₃. Crossover empirically determined; rated `[aspirational]`.

## 12. Lean verification

Two proofs, each requiring an approved sketch before implementation per CLAUDE.md §Verification work:

### V1 — bipedal F₃ arithmetic correctness

Statement (informal): for all `(m₁, s₁), (m₂, s₂) ∈ F₂⁶⁴ × F₂⁶⁴`, the bipedal `add`/`sub`/`mul`/`div` formulas applied to the pair, then decoded via ψ, equal canonical `Fp<3>` add/sub/mul/div on the decoded scalars, lane-wise.

Sketch (D2) lists the per-operation lemma form, the tactic for each (largely `bv_decide` for the 64-lane truth-table verification, with `decide` fallbacks at single-bit), the exact Rust path to be Charon-extracted (`Bipedal3::{add,sub,mul,div}`), and the expected Aeneas-generated def names.

### V2 — Ryser's formula

Statement (informal, bounded): for any `M : Matrix (Fp 3) n n` with `n ≤ 63`, `permanent_ryser_fp3(M) = perm(M)` where `perm` is the Lean-native Mathlib `Matrix.permanent`.

The bounded-n scope is deliberate — proving the unbounded case requires formalising arbitrary-arity sums and would explode the proof scope. Sketch (D3) explicitly says this. If the user wants unbounded, that's a follow-up epic.

V2 depends on the Rust API freezing (the `gate:api-freeze` gate before W6) because Charon re-extraction breaks on signature churn.

V3 (Lean F₅/F₇) is `[aspirational]` — the proof shape depends on the chosen encodings, which won't be settled until R1/R2 close. The sketches for V3 only get written after R1/R2 close.

## 13. Wave plan

The W*/T*/D*/R*/S*/V* IDs below are scratch labels for this design doc only — JIT issue titles will be clean per CLAUDE.md feedback memory.

### W0 — Decisions & sketches (parallel; user-approval gates marked ⚠)

| ID  | Title                                                              | Type        |
|-----|--------------------------------------------------------------------|-------------|
| D1a | Crate boundary: gf2-algebra vs gf2-core split                       | task        |
| D1b | Public API: PackedField<F> + Permanent trait surface ⚠              | task        |
| D1c | Feature-gate matrix (simd, parallel, hip, f5, f7)                   | task        |
| D2  | Lean4 sketch — bipedal F₃ correctness vs Fp<3> ⚠                   | task        |
| D3  | Lean4 sketch — Ryser's formula on FiniteField (n≤63 bounded) ⚠     | task        |
| D4  | MSRV 1.95 intrinsic feasibility (AVX2 + AVX-512 stubs)              | task        |
| R1  | F₅ packed encoding research (dev/research/f5_packing) ⚠            | simulation  |
| R2  | F₇ packed encoding research (dev/research/f7_packing) ⚠            | simulation  |
| R3  | Multi-word streaming column-sum cache-blocking design               | task        |
| R4  | SIMD batching strategy: generic vs per-prime (microbench)           | simulation  |

Dependency edges within W0:
- D1a → D1b → D1c
- (no other internal edges; the rest run truly in parallel)

### W1 — Foundation (after W0 closes for D1*)

| ID | Title                                                  |
|----|--------------------------------------------------------|
| T1 | Create gf2-algebra crate skeleton + workspace member   |
| T2 | PackedField trait + scalar reference impl over Fp<3>   |
| T3 | Bipedal3 element with paper bitwise formulas + tests   |
| T4 | Bipedal3Vec over Vec<u64> with mask_tail invariant     |
| T5 | Bipedal3Matrix row-major + transpose tests             |
| T6 | gray_code_iter shared subset enumerator                |

Edges: T1 → {T2, T3, T6}; T3 → T4 → T5; T2 ⫫ T3 (independent).

### W2 — Reference implementations + reproduction harness

| ID  | Title                                                                |
|-----|----------------------------------------------------------------------|
| T7  | Generic permanent_ryser<F: FiniteField>                              |
| T8  | permanent_mod3_reference (faithful Rust port of paper Julia)         |
| T9  | permanent_bipedal3 single-u64 fast path                              |
| T10 | Criterion benchmark suite skeleton (benches/permanent.rs)            |
| T11 | Test-vector suite (small known permanents + 1k-random cross-check)   |
| Sa  | (sim) Reproduce paper scaling slope (log-time vs n) within ±10%      |

Edges: T6 → {T7, T9}; T7 → {T8, T11}; T9 → T10; T10 → Sa; T8 → Sa.

### W3 — SIMD + multi-word + parallel

| ID  | Title                                                                              |
|-----|------------------------------------------------------------------------------------|
| T12 | SIMD bipedal3 kernel in gf2-kernels-simd (per R4 outcome; AVX2 + AVX-512 paths)    |
| T13 | gf2-algebra SIMD dispatch wiring + scalar fallback test                            |
| T14 | Multi-word streaming column-sum (n > 64) per R3 design                             |
| T15 | Rayon parallel permanent_bipedal3 with chunk-size sweep                            |
| S1  | (sim) ≥50× single-thread vs T8 at n=36 [hard]; vs paper Julia [aspirational]       |
| S2  | (sim) Parallel scaling 1..N cores [hard ≥0.85× linear, n≥28]                       |
| S3  | (sim) Cross-CPU portability sweep (AVX2-only host vs AVX-512 host)                 |

Edges: {R4, T9, D4} → T12 → T13 → {S1, S3}; {T9, R3} → T14; T9 → T15; T15 → S2.

### W4 — F₅ / F₇ (after R1/R2 close)

| ID  | Title                                                         |
|-----|---------------------------------------------------------------|
| T16 | Generic BatchedBipedalLike<P> framework (if R4 picks generic) |
| T17 | F₅ packed type + ops per R1                                   |
| T18 | permanent_bipedal5                                            |
| T19 | F₇ packed type + ops per R2                                   |
| T20 | permanent_bipedal7                                            |
| T21 | SIMD F₅ + F₇ kernels                                          |
| S4  | (sim) F₅/F₇ cross-validation vs external CAS (Sage/Magma)     |

Edges: {R1, T16} → T17 → T18; {R2, T16} → T19 → T20; {T17, T19} → T21; {T18, T20} → S4.

### W5 — GPU (gated on W3 closing + S2)

| ID  | Title                                                |
|-----|------------------------------------------------------|
| T22 | gf2-kernels-hip/hip/permanent/ scaffold              |
| T23 | F₃ HIP device kernel                                 |
| T24 | F₅ HIP device kernel                                 |
| T25 | F₇ HIP device kernel                                 |
| T26 | Host-side GPU dispatcher in gf2-algebra::gpu         |
| S5  | (sim) GPU vs CPU SIMD crossover; [aspirational] n≥40 |

Edges: {T13, T16} → T22 → T23 → T26; T22 → T24; T22 → T25; {T18, T24} → T26; {T20, T25} → T26; {T26, S1} → S5.

### W6 — Lean verification (gated on api-freeze)

| ID  | Title                                                           |
|-----|-----------------------------------------------------------------|
| Gf  | gate:api-freeze on gf2-algebra public surface                   |
| V1  | Lean proof — bipedal F₃ correctness per D2 sketch               |
| V2  | Lean proof — Ryser bounded n≤63 per D3 sketch                   |
| V3  | (aspirational) Lean F₅/F₇ correctness                           |

Edges: {T13, T15} → Gf → {V1, V2}; {V1, R1, R2} → V3.

### W7 — Reporting

| ID  | Title                                                                  |
|-----|------------------------------------------------------------------------|
| T27 | Plot generation scripts (plotters / matplotlib)                        |
| S6  | (sim) Publication-grade benchmark artefact, hardware + seed pinned     |
| T28 | scripts/permanent-repro.sh one-command reproduction                    |
| T29 | gf2-algebra README + doc-test examples                                 |
| T30 | Update root CLAUDE.md + ROADMAP.md + workspace docs                    |

Edges: T10 → T27; {S1, S2, S3, S4, S5} → S6; S6 → T28; {T13, T15} → T29; T29 → T30.

**Total child count:** 48.

## 14. Success criteria (epic level)

Per CLAUDE.md §Success-criterion maturity markers, `[hard]` = default; `[aspirational]` = empirically-amendable target.

1. **[hard]** A new workspace crate `gf2-algebra` exists, builds clean under `cargo nextest run --workspace --all-features --release --profile ci`, and is referenced from the workspace `Cargo.toml`.

2. **[hard]** `gf2-algebra::permanent::permanent_bipedal3` produces values bit-identical to `permanent_ryser` over `Fp<3>` on 1000 random matrices for every n ∈ {1..16} and on 100 random matrices for n=20, 24, 28, 32.

3. **[hard]** `gf2-algebra::permanent::permanent_bipedal3` (single-thread, AVX2 path) runs n=36 random {-1,0,1} matrices ≥ 50× faster than the in-tree `permanent_mod3_reference` on the same machine.

4. **[aspirational]** Same comparison vs the paper's published Julia number on a documented Zen 4/5 reference host: ≥ 50× speedup.

5. **[hard for n ≥ 28]** `permanent_bipedal3` parallel scaling factor is ≥ 0.85× per physical core up to the host's physical core count, measured by S2.

6. **[hard]** F₅ and F₇ packed permanents return values matching `permanent_ryser` over the corresponding `Fp<P>` on 1000 random matrices for n ∈ {1..14}.

7. **[hard]** Lean4 proof V1 builds with `lake build` and contains no `sorry`.

8. **[hard]** Lean4 proof V2 builds with `lake build`, scoped to bounded n ≤ 63, contains no `sorry`.

9. **[hard]** A reproducible benchmark artefact (S6) exists in `dev/benchmarks/gf2_algebra_permanent/`, with criterion JSONs, hardware fingerprint, seed pins, and `scripts/permanent-repro.sh` runs end-to-end.

10. **[aspirational]** GPU permanent crossover at n ≥ 40 for F₃ on gfx1030 (S5).

11. **[hard]** Root `CLAUDE.md` and `ROADMAP.md` are updated to reference `gf2-algebra`.

12. **[hard]** `cargo run -p gf2-algebra --example permanent_demo --release` produces the headline numbers from S6 within ±5%.

## 15. Risk register

| # | Risk                                                          | Likelihood | Impact | Mitigation                                                                                         |
|---|---------------------------------------------------------------|------------|--------|----------------------------------------------------------------------------------------------------|
| 1 | F₅/F₇ research yields no novel encoding beating LUT baseline  | Medium     | High   | Hard fallback documented in §8; W5 GPU still proceeds; degrade S4 expectations not S6.             |
| 2 | 50× target infeasible against paper Julia                     | Medium     | High   | Reformulated: [hard] is vs T8 in-tree port; vs Julia is [aspirational].                            |
| 3 | Lean Ryser proof scope explodes                               | High       | High   | D3 sketch scopes to bounded n ≤ 63; escalate if review > 2 cycles per CLAUDE.md verification rules.|
| 4 | GPU underperforms CPU SIMD at the n that matters              | Medium     | Medium | S5 already [aspirational]; CPU SIMD is publication headline.                                       |
| 5 | AVX-512 absent on dev box (CONFIRMED 5900X)                   | Certain    | Medium | AVX-512 paths gated [aspirational] from the start; CI runs AVX2 only on this host.                 |
| 6 | Rayon overhead eats parallelism on small n                    | High       | Medium | S2 includes chunk-size sweep; linear-scaling [hard] scoped to n ≥ 28.                              |
| 7 | Bipedal3 API churn after W1 forces W2/W3 rework               | Medium     | High   | D1b user-reviewed before W1; api-freeze gate before W6.                                            |
| 8 | Charon/Aeneas extraction breaks on generic Rust               | Medium     | High   | V2 proves the monomorphised `permanent_ryser_fp3`; D3 sketch flags this explicitly.                |
| 9 | Multi-word streaming cache pathology at large n               | Medium     | Medium | T14 success criterion includes a roofline analysis using the existing ppc-parallel skill.          |
| 10| Paper algorithm reproduction floor in Rust > paper Julia time | Low        | Low    | We measure T8 baseline empirically; S1's [hard] ratio adapts to whatever T8 actually clocks.       |

## 16. Appendix A — paper-to-Rust algorithm mapping (T8 input)

The paper's `permanent_mod3` (Listing 3, paper §3) ports to Rust as `permanent_mod3_reference` in `gf2-algebra::permanent::reference`. Key correspondences:

| Julia                          | Rust                                                  |
|--------------------------------|-------------------------------------------------------|
| `UInt64`                       | `u64`                                                 |
| `Tuple{UInt64, UInt64}`        | `(u64, u64)` or `Bipedal3` (debate in D1b)            |
| Gray code via `xor` of indices | `gf2_algebra::gray::gray_code_iter`                   |
| `popcount`                     | `u64::count_ones`                                     |
| `mod 3` reduction via centering| `Fp<3>::from_signed` (existing in gf2-core)           |

T8 produces a faithful port that is *not* SIMD-optimised — it's the baseline against which S1's [hard] 50× is measured. **It should be roughly slower than the paper's Julia by a small constant**, since Julia's runtime is competitive with Rust for tight bitops. If T8 measures *faster* than the paper's Julia, S1's [hard] ratio still applies vs whatever T8 clocks, and the [aspirational] vs-paper number gets a free boost.

## 17. Appendix B — paper Table 2 (verbatim)

Already in §2.4. Copied here for convenience:

| n  | permanent_Ryser (s) | permanent_mod3 (s) | ratio |
|----|---------------------|--------------------|-------|
| 24 | 1.96                | 0.025              | 78.4  |
| 26 | 7.98                | 0.099              | 80.6  |
| 28 | 32.3                | 0.401              | 80.5  |
| 30 | 131.5               | 1.59               | 82.7  |
| 32 | 533.5               | 6.34               | 84.1  |
| 34 | 2166.9              | 25.52              | 84.9  |
| 36 | 8857.9              | 101.9              | 86.9  |

The ratio creeps upward with n — consistent with `permanent_mod3` having a smaller per-step constant than `permanent_Ryser` and a 2ⁿ scaling that hides the growth.

## 18. References

- Scheinerman, D. *Fast computation of permanents over F₃ via F₂ arithmetic*. arxiv 2407.20205v2, August 2024.
- Ryser, H. J. *Combinatorial Mathematics*. Carus Mathematical Monograph No. 14, MAA, 1963.
- Glynn, D. G. *The permanent of a square matrix*. European Journal of Combinatorics 31 (2010).
- gf2 project: `CLAUDE.md`, `dev/plans/gf2_core_ppc_spiral.md`, `dev/plans/quadratic_ext.md`.

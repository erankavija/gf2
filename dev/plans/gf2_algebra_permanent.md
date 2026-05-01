# gf2-algebra: fast matrix permanents over $\mathbb{F}_3 / \mathbb{F}_5 / \mathbb{F}_7$

**Epic slug:** `epic:gf2-algebra-permanent`
**Status:** design / $W_0$
**Reference paper:** Danny Scheinerman, *Fast computation of permanents over $\mathbb{F}_3$ via $\mathbb{F}_2$ arithmetic*, arxiv 2407.20205v2 (Aug 2024).

## 1. Motivation

The reference paper introduces a "bipedal" map encoding $\mathbb{F}_3^n$ as a pair of $\mathbb{F}_2^n$ vectors and uses bitwise primitives to implement add/sub/mul/div in 1–6 ops each. Combined with Ryser's permanent formula in Gray-code order, the resulting `permanent_mod3` routine achieves a reported $86.9\times$ speedup over a naive Ryser implementation in Julia, single-threaded, on a 4.20 GHz desktop. The paper does not exploit SIMD, multi-thread parallelism, or GPU.

This epic sets out to:

1. Reproduce the paper's headline results in Rust (in-tree, deterministic).
2. **Beat** them by $\ge 50\times$ single-thread vs the in-tree Rust port, with linear parallel scaling on multicore, and demonstrate competitive GPU performance at large $n$.
3. Generalise the bipedal idea to $\mathbb{F}_5$ and $\mathbb{F}_7$ via best-effort encoding research.
4. Mechanically verify (Lean4 via Charon/Aeneas) the bipedal $\mathbb{F}_3$ arithmetic and Ryser's formula on the project's `FiniteField` trait.
5. Produce a publication-grade benchmark artefact (criterion + plots + reproducible scripts).

The technique fits the project's research-grade-toolkit charter (CLAUDE.md §Vision). All required infrastructure — `Vec<u64>`-backed bit storage with `mask_tail`, AVX2/AVX-512 logical kernels, a rayon CPU backend, an HIP/ROCm GPU crate, and a Charon/Aeneas Lean pipeline — already exists in the workspace.

## 2. Reference paper, in detail

### 2.1 Bipedal representation

Define $\varphi: \mathbb{F}_3 \to \mathbb{F}_2 \times \mathbb{F}_2$ by

$$
\varphi(0) = (0, 0), \quad \varphi(1) = (1, 0), \quad \varphi(-1) = (1, 1)
$$

extended pointwise to vectors. Each $\mathbb{F}_3^n$ value is represented as a pair $(\mathit{mag}, \mathit{sgn}) \in (\mathbb{F}_2^n)^2$. The decoder $\psi: \mathbb{F}_2 \times \mathbb{F}_2 \to \mathbb{F}_3$ also maps $(0, 1) \mapsto 0$ (alternative-zero), giving a 4-state encoding with one redundant codeword. This redundancy is what permits the cheap arithmetic.

### 2.2 Bipedal arithmetic (Theorem 2.1, paper)

For pairs $x = (m_1, s_1)$, $y = (m_2, s_2)$:

- **Multiplication** (2 ops):
  $$
  m_\times = m_1 \mathbin{\&} m_2, \qquad s_\times = s_1 \oplus s_2
  $$
- **Addition** (6 ops with CSE):
  $$
  t = m_1 \oplus s_1 \oplus s_2, \quad u = m_2 \mathbin{\&} t, \quad m_+ = u \mathbin{|} (m_1 \oplus m_2), \quad s_+ = u \oplus s_1
  $$
- **Subtraction** (6 ops):
  $$
  t = s_1 \oplus s_2, \quad u = m_1 \mathbin{\&} t, \quad m_- = u \mathbin{|} (m_1 \oplus m_2), \quad s_- = u \oplus (m_2 \oplus s_2)
  $$
- **Division by nonzero** (1 op): $m_\div = m_1$, $s_\div = s_1 \oplus s_2$.

These operations are lane-parallel: a single 64-bit AND/XOR processes 64 independent $\mathbb{F}_3$ elements. `gf2-kernels-simd::LogicalFns` exposes vectorised AND/XOR/OR/POPCNT over `&mut [u64]`, so a packed `Bipedal3Vec` of any length composes directly without writing a new kernel for the basic ops.

### 2.3 Ryser's formula

$$
\operatorname{perm}(A) = (-1)^n \sum_{S \subseteq \{1, \ldots, n\}} (-1)^{|S|} \prod_{i=1}^{n} \left( \sum_{j \in S} a_{ij} \right)
$$

Naive enumeration is $O(n^2 \cdot 2^n)$. Iterating subsets in Gray-code order — flipping exactly one bit per step — reduces the column-sum update to a single vector add/sub, giving $O(n \cdot 2^n)$ field operations. With bipedal packing, one $\mathbb{F}_3$ vector add or subtract is $O(\lceil n/64 \rceil)$ word ops. For $n \le 64$ this is one word op, so the inner loop is **effectively $O(2^n)$ primitive `u64` operations**.

### 2.4 Paper Table 2 (verbatim)

Single-thread Julia, 4.20 GHz desktop, time in seconds:

| $n$ | `permanent_Ryser` | `permanent_mod3` |
|-----|-------------------|------------------|
| 24  |          1.96     |          0.025   |
| 26  |          7.98     |          0.099   |
| 28  |         32.3      |          0.401   |
| 30  |        131.5      |          1.59    |
| 32  |        533.5      |          6.34    |
| 34  |       2166.9      |         25.52    |
| 36  |       8857.9      |        101.9     |

Headline ratio at $n=36$: **$86.9\times$**. Parallel demo: $n=50$ in 832 CPU-hours on 128 cores (a different, large-scale benchmark not in Table 2).

### 2.5 Secondary claim

Monte Carlo evidence in the paper suggests $\operatorname{perm}(A)$ tends to the uniform distribution on $\mathbb{F}_3$ as $n \to \infty$ for random $\{-1, 0, 1\}$ matrices. This is a `type:simulation` reproduction target.

## 3. Scope and non-goals

### In scope

- Bipedal $\mathbb{F}_3$ packed types (element, vector, matrix) with paper's bitwise formulas.
- Generic `permanent_ryser<F: FiniteField>` reference implementation.
- Specialised `permanent_bipedal3` single- and multi-word fast paths.
- AVX2 SIMD acceleration on Zen 3 (the dev host); AVX-512 path coded but `[aspirational]`-rated since the dev host lacks AVX-512.
- Rayon parallel permanent with chunk-size sweep.
- $\mathbb{F}_5$ and $\mathbb{F}_7$ packed types via best-effort encoding research; permanents over those fields.
- HIP/ROCm GPU permanent kernels (gfx1030).
- Lean4 mechanical proofs of (a) bipedal $\mathbb{F}_3$ arithmetic correctness vs `Fp<3>`, (b) Ryser's formula on `FiniteField`, scoped to the bounded $n \le 63$ single-`u64` regime.
- Publication-grade benchmark artefact (criterion + plots + reproducible scripts).
- Monte Carlo distribution-of-perm verification.

### Non-goals

- No general-purpose char-3 linear algebra (det, rank, char-poly, Smith normal form). These are natural follow-ups but out of scope for this epic.
- No char-2 permanents (over $\mathbb{F}_2$, $\operatorname{perm} = \det$, already trivial via existing rank/RREF code).
- No primes $p \ge 11$ beyond what falls out for free.
- No proof of unbounded-$n$ Ryser (would require formalising arbitrary-arity sums).
- No reproduction of the paper's $n=50$ / 832 CPU-hour run on 128 cores — a smaller analogous run will demonstrate the speedup factor.

## 4. Hardware envelope

Dev host (verified 2026-05-01): AMD Ryzen 9 5900X (Zen 3, 12-core / 24-thread). Flags include AVX2, FMA, AES-NI, VAES, VPCLMULQDQ, SSE4.2, BMI1/BMI2, **but no AVX-512**.

Implications:

- Headline single-thread number: AVX2-based bipedal kernel, 256-bit lanes ($4 \times $ `u64`).
- AVX-512 paths must be coded (for portability) but rated `[aspirational]`. CI runs only the AVX2 path on this host.
- 12 physical cores $\Rightarrow$ the parallel-scaling target is $0.85 \times 12 \approx 10\times$ over single-thread. With SMT, a stretch goal is $\ge 12\times$.
- GPU host: existing gf2-kernels-hip target gfx1030 (RX 6000 series). Same host, opt-in `--features hip`.

The 4.20 GHz Julia desktop in the paper is unspecified. **Direct cross-language cross-machine comparison is unreliable**; the `[hard]` perf criterion targets in-tree Rust-vs-Rust. Vs Julia is `[aspirational]`.

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
│   │   ├── packed5.rs  # F_5 packed (post-encoding decision)
│   │   └── packed7.rs  # F_7 packed (post-encoding decision)
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

- `gf2-algebra` $\to$ `gf2-core` (for `FiniteField`, `BitVec`, `Fp<P>`)
- `gf2-algebra` $\to$ `gf2-kernels-simd` (feature-gated `simd`)
- `gf2-algebra` $\to$ `gf2-kernels-hip` (feature-gated `hip`; will not be in default workspace)

`#![deny(unsafe_code)]` everywhere. SIMD code (`unsafe`) lives in `gf2-kernels-simd::bipedal_kernel`. GPU code (`unsafe`) lives in `gf2-kernels-hip::permanent`.

## 6. The `PackedField<F>` trait

**Critical design point.** A `Bipedal3` value packs many $\mathbb{F}_3$ elements into two `u64`s; it is *not* a single field element and **must not** implement `FiniteField`. Instead `gf2-algebra::packed::PackedField<F>` is a new trait abstracting "lane-parallel arithmetic over an underlying scalar field":

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

`Bipedal3` $:$ `PackedField<Fp<3>>` with `LANES = 64`. There is *also* a separate `PackedFieldVec<F>` interface for variable-length packed vectors (a `Vec<u64>` pair under the hood) — that is what `Bipedal3Vec` and `Bipedal3Matrix` build on. The exact type signatures are settled in the W0 trait-surface decision before W1 begins; this section is the strawman input.

The `Permanent` trait sits next to `PackedField`:

```rust
pub trait Permanent<F: FiniteField> {
    type Matrix;
    fn permanent(&self) -> F;
}
```

Implemented for:

- generic dense matrices via `permanent_ryser<F>`
- `Bipedal3Matrix` via `permanent_bipedal3`
- `Bipedal5Matrix`, `Bipedal7Matrix`

Dispatch (SIMD, parallel, GPU) is internal to each `permanent_*` impl, gated by features.

## 7. $\mathbb{F}_3$ bipedal — exact code shape

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

The element-type implementation is property-tested (`proptest`, 10k random pairs, all ops match `Fp<3>`) and exhaustive lane-truth-table tested for $n \le 16$.

### 7.2 Vec and Matrix

`Bipedal3Vec` $=$ pair of `Vec<u64>` with shared length, `mask_tail` invariant per CLAUDE.md §Key design invariants. Word-boundary tests at lengths $\{0, 1, 63, 64, 65, 127, 128, 129\}$.

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

The `fold_mul` reduction is a log-tree: `mul` has 2 ops per step, total $2 \cdot \log_2(n)$ `u64` ops. For $n=36$ that is $\approx 12$ ops total per Gray step.

The Gray-code iterator is shared with generic `permanent_ryser<F>` — no duplication.

## 8. $\mathbb{F}_5$ / $\mathbb{F}_7$ encoding — research bounds

**Thesis to test.** $\mathbb{F}_5$ and $\mathbb{F}_7$ admit "bipedal-like" multi-bit encodings minimising the bitwise op count for add/mul. The simplest baseline is 3-bit-per-element packed (16 elements per `u64`) with a $2^{16}$-entry table-LUT for mul. Better encodings may exist by exploiting the multiplicative structure ($\mathbb{F}_5^*$ is cyclic of order 4; $\mathbb{F}_7^*$ of order 6) — e.g., a $(\log, \mathit{sign})$ split for $\mathbb{F}_5$, or a specialised redundant 4-bit code for $\mathbb{F}_7$.

The two encoding-research issues are `type:simulation` and prototype candidate encodings in `dev/research/{f5,f7}_packing/` (mirroring `dev/research/rns_prototype/`), measure cycles/op, and produce a decision document selecting the winning candidate. **User approval required** before they close — these are best-effort research items where the encoding choice has wide impact on the implementation phase.

**Hard fallback** if the research issues do not yield novel encodings beating the LUT baseline: 3-bit-per-element packed + $2^{16}$ LUT mul. This still gives a useful speedup on top of `Fp<5>`/`Fp<7>` Montgomery arithmetic and lets the GPU phase proceed without blocking.

## 9. Multi-word streaming column-sum ($n > 64$)

For $n > 64$ the column-sum vector spans multiple `u64`-pairs. The Ryser inner loop becomes:

```
column_sum[w] = column_sum[w] ± A[w][flip]   for w in 0..ceil(n/64)
prod = fold_mul over all words
```

`A[w][flip]` is the `flip`-th column's `w`-th word — i.e., we walk down a column. To make this cache-friendly the matrix is stored in **column-major** order for the multi-word path: `A.col(j)` is contiguous. (Single-word path is column-major too; this is a uniform layout.) Cache-block by L1 footprint so the column-sum vector and the touched column both fit. The streaming-design issue produces the cache-blocking decision.

For $n \in \{65, \ldots, 256\}$, multi-word streaming is verified correct against a naive multi-precision reference. Beyond 256 we fall back to the parallel/GPU path because single-thread time grows like $2^n$ regardless.

## 10. SIMD batching strategy

Two strategies, picked by the SIMD-batching microbench issue:

- **Per-prime hand-rolled.** A `bipedal3_kernel.rs` writes specialised AVX2/AVX-512 functions for `Bipedal3` vectors; $\mathbb{F}_5$ and $\mathbb{F}_7$ each get their own kernel files. Maximum performance per prime, more code.
- **Generic `BatchedBipedalLike<P, MagLanes, SgnLanes>` framework.** A single SIMD primitive parametrised over the encoding shape; $\mathbb{F}_3$, $\mathbb{F}_5$, $\mathbb{F}_7$ instantiate the same kernel template. Less code, may sacrifice constant factor.

The batching issue measures both and picks one. The decision is logged in `dev/plans/r4_simd_batching_decision.md` and attached to the issue. Either way, the W3 SIMD-kernel issue commits to the choice for $\mathbb{F}_3$ first, then the W4 SIMD-kernel issue extends to $\mathbb{F}_5$/$\mathbb{F}_7$.

## 11. GPU strategy

`gf2-kernels-hip/hip/permanent_*.hip` device kernels, opt-in via `--features hip`. Each kernel takes a flat `(mag[], sgn[])` representation of the column-sum buffer and a chunk of Gray-code work indices, and accumulates the partial signed product sum into a per-thread accumulator. Host-side reduction in `gf2-algebra::gpu`.

The work decomposition mirrors the rayon design (block-of-Gray-steps per work unit), so once the W3 phase settles the chunking strategy, W5 reuses the same partition logic. **W5 is gated on W3 + parallel-scaling closing** for that reason.

GPU performance target: GPU wins over CPU SIMD at $n \ge 40$ for $\mathbb{F}_3$. Crossover empirically determined; rated `[aspirational]`.

## 12. Lean verification

Two proofs, each requiring an approved sketch before implementation per CLAUDE.md §Verification work:

### V1 — bipedal $\mathbb{F}_3$ arithmetic correctness

Statement (informal): for all $(m_1, s_1), (m_2, s_2) \in \mathbb{F}_2^{64} \times \mathbb{F}_2^{64}$, the bipedal `add`/`sub`/`mul`/`div` formulas applied to the pair, then decoded via $\psi$, equal canonical `Fp<3>` add/sub/mul/div on the decoded scalars, lane-wise.

The sketch lists the per-operation lemma form, the tactic for each (largely `bv_decide` for the 64-lane truth-table verification, with `decide` fallbacks at single-bit), the exact Rust path to be Charon-extracted (`Bipedal3::{add, sub, mul, div}`), and the expected Aeneas-generated def names.

### V2 — Ryser's formula

Statement (informal, bounded): for any $M \in \mathrm{Matrix}(\mathbb{F}_3, n, n)$ with $n \le 63$, `permanent_ryser_fp3(M)` $= \operatorname{perm}(M)$ where $\operatorname{perm}$ is the Lean-native Mathlib `Matrix.permanent`.

The bounded-$n$ scope is deliberate — proving the unbounded case requires formalising arbitrary-arity sums and would explode the proof scope. The sketch explicitly says this. If unbounded is wanted, that is a follow-up epic.

V2 depends on the Rust API freezing (the `gate:api-freeze` gate before W6) because Charon re-extraction breaks on signature churn.

V3 (Lean $\mathbb{F}_5$/$\mathbb{F}_7$) is `[aspirational]` — the proof shape depends on the chosen encodings, which are not settled until the research issues close. The sketches for V3 only get written after those close.

## 13. Wave plan

The $W_*/T_*/D_*/R_*/S_*/V_*$ IDs below are scratch labels for this design doc only — JIT issue titles are clean per CLAUDE.md feedback memory.

### $W_0$ — Decisions & sketches (parallel; user-approval gates marked ⚠)

| ID  | Title                                                              | Type        |
|-----|--------------------------------------------------------------------|-------------|
| D1a | Crate boundary: gf2-algebra vs gf2-core split                       | task        |
| D1b | Public API: PackedField + Permanent trait surface ⚠                 | task        |
| D1c | Feature-gate matrix (simd, parallel, hip, f5, f7)                   | task        |
| D2  | Lean4 sketch — bipedal F_3 correctness vs canonical prime field ⚠   | task        |
| D3  | Lean4 sketch — Ryser's formula on FiniteField ($n \le 63$ bounded) ⚠| task        |
| D4  | MSRV 1.95 intrinsic feasibility (AVX2 + AVX-512 stubs)              | task        |
| R1  | F_5 packed encoding research (dev/research/f5_packing) ⚠            | simulation  |
| R2  | F_7 packed encoding research (dev/research/f7_packing) ⚠            | simulation  |
| R3  | Multi-word streaming column-sum cache-blocking design               | task        |
| R4  | SIMD batching strategy: generic vs per-prime (microbench)           | simulation  |

Dependency edges within $W_0$: $D_{1a} \to D_{1b} \to D_{1c}$, $D_{1b} \to R_4$. The rest run truly in parallel.

### $W_1$ — Foundation (after $W_0$ closes for $D_{1\ast}$)

| ID  | Title                                                  |
|-----|--------------------------------------------------------|
| T1  | Create gf2-algebra crate skeleton + workspace member   |
| T2  | PackedField trait + scalar reference impl over Fp<3>   |
| T3  | Bipedal3 element with paper bitwise formulas + tests   |
| T4  | Bipedal3Vec over Vec<u64> with mask_tail invariant     |
| T5  | Bipedal3Matrix row-major + transpose tests             |
| T6  | gray_code_iter shared subset enumerator                |

Edges: $T_1 \to \{T_2, T_3, T_6\}$; $T_3 \to T_4 \to T_5$; $T_2 \perp T_3$ (independent).

### $W_2$ — Reference implementations + reproduction harness

| ID  | Title                                                                |
|-----|----------------------------------------------------------------------|
| T7  | Generic permanent_ryser<F: FiniteField>                              |
| T8  | permanent_mod3_reference (faithful Rust port of paper Julia)         |
| T9  | permanent_bipedal3 single-u64 fast path                              |
| T10 | Criterion benchmark suite skeleton (benches/permanent.rs)            |
| T11 | Test-vector suite (small known permanents + 1k-random cross-check)   |
| Sa  | (sim) Reproduce paper scaling slope (log-time vs $n$) within ±10%    |

Edges: $T_6 \to \{T_7, T_9\}$; $T_7 \to \{T_8, T_{11}\}$; $T_9 \to T_{10}$; $T_{10} \to S_a$; $T_8 \to S_a$.

### $W_3$ — SIMD + multi-word + parallel

| ID  | Title                                                                                |
|-----|--------------------------------------------------------------------------------------|
| T12 | SIMD bipedal3 kernel in gf2-kernels-simd (per R4 outcome; AVX2 + AVX-512 paths)      |
| T13 | gf2-algebra SIMD dispatch wiring + scalar fallback test                              |
| T14 | Multi-word streaming column-sum ($n > 64$) per R3 design                             |
| T15 | Rayon parallel permanent_bipedal3 with chunk-size sweep                              |
| S1  | (sim) $\ge 50\times$ ST vs T8 at $n=36$ [hard]; vs paper Julia [aspirational]        |
| S2  | (sim) Parallel scaling 1..N cores [hard $\ge 0.85\times$ linear, $n \ge 28$]         |
| S3  | (sim) Cross-CPU portability sweep (AVX2-only host vs AVX-512 host)                   |

Edges: $\{R_4, T_9, D_4\} \to T_{12} \to T_{13} \to \{S_1, S_3\}$; $\{T_9, R_3\} \to T_{14}$; $T_9 \to T_{15}$; $T_{15} \to S_2$.

### $W_4$ — $\mathbb{F}_5$ / $\mathbb{F}_7$ (after $R_1$/$R_2$ close)

| ID  | Title                                                         |
|-----|---------------------------------------------------------------|
| T16 | Generic BatchedBipedalLike<P> framework (if R4 picks generic) |
| T17 | F_5 packed type + ops per R1                                  |
| T18 | permanent_bipedal5                                            |
| T19 | F_7 packed type + ops per R2                                  |
| T20 | permanent_bipedal7                                            |
| T21 | SIMD F_5 + F_7 kernels                                        |
| S4  | (sim) F_5/F_7 cross-validation vs external CAS (Sage/Magma)   |

Edges: $\{R_1, T_{16}\} \to T_{17} \to T_{18}$; $\{R_2, T_{16}\} \to T_{19} \to T_{20}$; $\{T_{17}, T_{19}\} \to T_{21}$; $\{T_{18}, T_{20}\} \to S_4$.

### $W_5$ — GPU (gated on $W_3$ closing + $S_2$)

| ID  | Title                                                  |
|-----|--------------------------------------------------------|
| T22 | gf2-kernels-hip/hip/permanent/ scaffold                |
| T23 | F_3 HIP device kernel                                  |
| T24 | F_5 HIP device kernel                                  |
| T25 | F_7 HIP device kernel                                  |
| T26 | Host-side GPU dispatcher in gf2-algebra::gpu           |
| S5  | (sim) GPU vs CPU SIMD crossover; [aspirational] $n \ge 40$ |

Edges: $\{T_{13}, T_{16}\} \to T_{22} \to T_{23} \to T_{26}$; $T_{22} \to T_{24}$; $T_{22} \to T_{25}$; $\{T_{18}, T_{24}\} \to T_{26}$; $\{T_{20}, T_{25}\} \to T_{26}$; $\{T_{26}, S_1\} \to S_5$.

### $W_6$ — Lean verification (gated on api-freeze)

| ID  | Title                                                               |
|-----|---------------------------------------------------------------------|
| Gf  | gate:api-freeze on gf2-algebra public surface                       |
| V1  | Lean proof — bipedal F_3 correctness per D2 sketch                  |
| V2  | Lean proof — Ryser bounded $n \le 63$ per D3 sketch                 |
| V3  | (aspirational) Lean F_5/F_7 correctness                             |

Edges: $\{T_{13}, T_{15}\} \to G_f \to \{V_1, V_2\}$; $\{V_1, R_1, R_2\} \to V_3$.

### $W_7$ — Reporting

| ID  | Title                                                                  |
|-----|------------------------------------------------------------------------|
| T27 | Plot generation scripts (plotters / matplotlib)                        |
| S6  | (sim) Publication-grade benchmark artefact, hardware + seed pinned     |
| T28 | scripts/permanent-repro.sh one-command reproduction                    |
| T29 | gf2-algebra README + doc-test examples                                 |
| T30 | Update root CLAUDE.md + ROADMAP.md + workspace docs                    |

Edges: $T_{10} \to T_{27}$; $\{S_1, S_2, S_3, S_4, S_5\} \to S_6$; $S_6 \to T_{28}$; $\{T_{13}, T_{15}\} \to T_{29}$; $T_{29} \to T_{30}$.

**Total child count:** 48.

## 14. Success criteria (epic level)

Per CLAUDE.md §Success-criterion maturity markers, `[hard]` $=$ default; `[aspirational]` $=$ empirically-amendable target.

1. **[hard]** A new workspace crate `gf2-algebra` exists, builds clean under `cargo nextest run --workspace --all-features --release --profile ci`, and is referenced from the workspace `Cargo.toml`.
2. **[hard]** `gf2-algebra::permanent::permanent_bipedal3` produces values bit-identical to `permanent_ryser` over `Fp<3>` on 1000 random matrices for every $n \in \{1, \ldots, 16\}$ and on 100 random matrices for $n \in \{20, 24, 28, 32\}$.
3. **[hard]** `gf2-algebra::permanent::permanent_bipedal3` (single-thread, AVX2 path) runs $n=36$ random $\{-1, 0, 1\}$ matrices $\ge 50\times$ faster than the in-tree `permanent_mod3_reference` on the same machine.
4. **[aspirational]** Same comparison vs the paper's published Julia number on a documented Zen 4/5 reference host: $\ge 50\times$ speedup.
5. **[hard for $n \ge 28$]** `permanent_bipedal3` parallel scaling factor is $\ge 0.85\times$ per physical core up to the host's physical core count.
6. **[hard]** $\mathbb{F}_5$ and $\mathbb{F}_7$ packed permanents return values matching `permanent_ryser` over the corresponding `Fp<P>` on 1000 random matrices for $n \in \{1, \ldots, 14\}$.
7. **[hard]** Lean4 proof V1 builds with `lake build` and contains no `sorry`.
8. **[hard]** Lean4 proof V2 builds with `lake build`, scoped to bounded $n \le 63$, contains no `sorry`.
9. **[hard]** A reproducible benchmark artefact exists in `dev/benchmarks/gf2_algebra_permanent/`, with criterion JSONs, hardware fingerprint, seed pins, and `scripts/permanent-repro.sh` runs end-to-end.
10. **[aspirational]** GPU permanent crossover at $n \ge 40$ for $\mathbb{F}_3$ on gfx1030.
11. **[hard]** Root `CLAUDE.md` and `ROADMAP.md` are updated to reference `gf2-algebra`.
12. **[hard]** `cargo run -p gf2-algebra --example permanent_demo --release` produces the headline numbers within $\pm 5\%$.

## 15. Risk register

| #  | Risk                                                          | Likelihood | Impact | Mitigation                                                                                         |
|----|---------------------------------------------------------------|------------|--------|----------------------------------------------------------------------------------------------------|
| 1  | $\mathbb{F}_5$/$\mathbb{F}_7$ research yields no novel encoding beating LUT | Medium     | High   | Hard fallback documented in §8; GPU phase still proceeds; degrade S4 expectations not S6.          |
| 2  | $50\times$ target infeasible against paper Julia              | Medium     | High   | Reformulated: `[hard]` is vs in-tree port; vs Julia is `[aspirational]`.                          |
| 3  | Lean Ryser proof scope explodes                               | High       | High   | D3 sketch scopes to bounded $n \le 63$; escalate if review > 2 cycles per CLAUDE.md.               |
| 4  | GPU underperforms CPU SIMD at the $n$ that matters            | Medium     | Medium | S5 already `[aspirational]`; CPU SIMD is publication headline.                                     |
| 5  | AVX-512 absent on dev box (CONFIRMED 5900X)                   | Certain    | Medium | AVX-512 paths gated `[aspirational]` from the start; CI runs AVX2 only on this host.               |
| 6  | Rayon overhead eats parallelism on small $n$                  | High       | Medium | S2 includes chunk-size sweep; linear-scaling `[hard]` scoped to $n \ge 28$.                        |
| 7  | `Bipedal3` API churn after W1 forces W2/W3 rework             | Medium     | High   | D1b user-reviewed before W1; api-freeze gate before W6.                                            |
| 8  | Charon/Aeneas extraction breaks on generic Rust               | Medium     | High   | V2 proves the monomorphised `permanent_ryser_fp3`; D3 sketch flags this explicitly.                |
| 9  | Multi-word streaming cache pathology at large $n$             | Medium     | Medium | T14 success criterion includes a roofline analysis using the existing ppc-parallel skill.          |
| 10 | Paper algorithm reproduction floor in Rust > paper Julia time | Low        | Low    | T8 baseline measured empirically; S1's `[hard]` ratio adapts to whatever T8 actually clocks.       |

## 16. Appendix A — paper-to-Rust algorithm mapping

The paper's `permanent_mod3` (Listing 3, paper §3) ports to Rust as `permanent_mod3_reference` in `gf2-algebra::permanent::reference`. Key correspondences:

| Julia                          | Rust                                                  |
|--------------------------------|-------------------------------------------------------|
| `UInt64`                       | `u64`                                                 |
| `Tuple{UInt64, UInt64}`        | `(u64, u64)` or `Bipedal3` (debate in D1b)            |
| Gray code via `xor` of indices | `gf2_algebra::gray::gray_code_iter`                   |
| `popcount`                     | `u64::count_ones`                                     |
| `mod 3` reduction via centering| `Fp<3>::from_signed` (existing in gf2-core)           |

The reference port is *not* SIMD-optimised — it is the baseline against which the $50\times$ `[hard]` is measured. **It should be roughly slower than the paper's Julia by a small constant**, since Julia's runtime is competitive with Rust for tight bitops. If T8 measures *faster* than the paper's Julia, S1's `[hard]` ratio still applies vs whatever T8 clocks, and the `[aspirational]` vs-paper number gets a free boost.

## 17. Appendix B — paper Table 2 (verbatim, with ratios)

| $n$ | `permanent_Ryser` (s) | `permanent_mod3` (s) | ratio |
|-----|-----------------------|----------------------|-------|
| 24  | 1.96                  | 0.025                | 78.4  |
| 26  | 7.98                  | 0.099                | 80.6  |
| 28  | 32.3                  | 0.401                | 80.5  |
| 30  | 131.5                 | 1.59                 | 82.7  |
| 32  | 533.5                 | 6.34                 | 84.1  |
| 34  | 2166.9                | 25.52                | 84.9  |
| 36  | 8857.9                | 101.9                | 86.9  |

The ratio creeps upward with $n$ — consistent with `permanent_mod3` having a smaller per-step constant than `permanent_Ryser` and a $2^n$ scaling that hides the growth.

## 18. References

- Scheinerman, D. *Fast computation of permanents over $\mathbb{F}_3$ via $\mathbb{F}_2$ arithmetic*. arxiv 2407.20205v2, August 2024.
- Ryser, H. J. *Combinatorial Mathematics*. Carus Mathematical Monograph No. 14, MAA, 1963.
- Glynn, D. G. *The permanent of a square matrix*. European Journal of Combinatorics 31 (2010).
- gf2 project: `CLAUDE.md`, `dev/plans/gf2_core_ppc_spiral.md`, `dev/plans/quadratic_ext.md`.

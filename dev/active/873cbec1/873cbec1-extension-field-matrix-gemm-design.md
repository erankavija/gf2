# Extension-Field Matrix GEMM via Base-Field Decomposition

| Field         | Value                                                                             |
|---------------|-----------------------------------------------------------------------------------|
| Date          | 2026-05-25                                                                        |
| JIT issue     | 873cbec1-6c55-44db-9685-f6c8a2614eb6                                              |
| Parent epic   | 026fc832                                                                          |
| Predecessor   | 41096af5 (route-selection wave 4 — closed)                                        |
| Plan ref      | `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 4                      |

---

## Verbatim Success-Criteria Quote

> - [hard] A design document attached via `jit doc add` that:
>   - states the public API surface (trait additions, free functions, or method impls on `FieldMatrix`) needed to specialise matmul for `QuadraticExt<_>` and `CubicExt<_>`,
>   - shows the Karatsuba decomposition explicitly (3 base-field GEMMs + corresponding adds/subs for quadratic; the analogous formula for cubic via Toom-Cook or recursive Karatsuba),
>   - identifies the correctness tests required (round-trip equality vs the existing generic matmul for `QuadraticExt<Fp<P>>` and `CubicExt<Fp<P>>` at n in {16, 64, 256}),
>   - identifies the benchmark cells (`QuadraticExt<Fp<65537>>`, `CubicExt<Fp<65537>>`, and at least one extension over a smaller GF(p) base) that will measure the speedup,
>   - states the non-regression contract (no regression on existing extension-field paths used by other crates, e.g., `gf2-coding` or `gf2-algebra`).
> - [hard] The design cites the relevant existing files: `crates/gf2-core/src/gfpn/quadratic.rs`, `crates/gf2-core/src/gfpn/cubic.rs`, `crates/gf2-core/src/gfpn/batch.rs`, `crates/gf2-core/src/gfpn/ext_config.rs`, `crates/gf2-core/src/field/matrix.rs::gemm`.
> - [hard] The design lists the implementation child issues needed to land Phase 4 (separate JIT issues, with dependency order).
> - [hard] No production code changes in this task — it is design-only.

---

## Problem Statement

### Current state

`FieldMatrix<QuadraticExt<C>>` and `FieldMatrix<CubicExt<C>>` today use the generic `gemm` dispatch in `crates/gf2-core/src/field/matrix.rs` (line 2562). That dispatch:

1. transposes B,
2. attempts `try_simd_gemm_classical` (returns `false` for extension types),
3. attempts `try_gf2m_u64_batch_dot_product` (returns `None` for extension types),
4. attempts `try_fp_simd_dot_packed_u16` (returns `None` for extension types),
5. falls back to `dot_product_slices`, which calls `mul_to_wide` + `reduce_wide` per output element.

For a `QuadraticExt<C>` element, `mul_to_wide` (line 666 of `quadratic.rs`) performs the Karatsuba multiply at the scalar level — 3 base-field multiplications per output element — then accumulates the result into a `QuadraticExtWide` accumulator over the `k` inner-dimension terms. This is correct but it leaves the `k` dot-product accumulations inside the `dot_product_slices` scalar loop, walking memory one extension-field element at a time.

For an (m x k) * (k x n) GEMM, the total cost is:

- **Quadratic (generic path):** m*n dot-products, each with k Karatsuba scalar multiplies = 3*m*n*k base-field multiplications. Memory access is not contiguous by base-field component; the AoS layout means a0, a1 interleaved in each row.

- **Cubic (generic path):** m*n dot-products, each with k 6-mul scalar multiply = 6*m*n*k base-field multiplications.

### The gap

The now-optimized base-field GEMM for `Fp<65537>` (and other fast-path primes from 41096af5) is completely dormant for extension-field callers. The `FieldMatrix<QuadraticExt<Fp<65537>>>` case never triggers `try_simd_gemm_classical` because `QuadraticExt<Fp<65537>>` is not `Fp<65537>`.

A matrix-level Karatsuba decomposition lifts the problem: instead of doing Karatsuba element-by-element in the inner dot-product loop, we decompose the matrix product into a small number of base-field GEMMs that can themselves use the fast path.

---

## Karatsuba Decomposition -- Quadratic

### Scalar Karatsuba (existing, for reference)

For elements `A = a0 + a1*u`, `B = b0 + b1*u` with u^2 = beta:

```text
v0 = a0 * b0
v1 = a1 * b1
c0 = v0 + beta * v1
c1 = (a0 + a1) * (b0 + b1) - v0 - v1
```

This is 3 base-field multiplications. Source: `crates/gf2-core/src/gfpn/quadratic.rs`, lines 460-466 and doc header; formula attributed in the source to Devegili, O hEigeartaigh, Scott, Dahab, ePrint 2006/471.

Original discovery: Karatsuba, A.A. and Ofman, Yu, "Multiplication of Multi-Digit Numbers on Automata", Soviet Physics Doklady, 7:595-596, 1963.

### Matrix-level Karatsuba lift (new)

Let A = A0 + A1*u and B = B0 + B1*u where A0, A1, B0, B1 are m*k matrices over the base field, and u^2 = beta (a base-field scalar).

The product C = A * B is a matrix over `QuadraticExt<C>` with components:

```
C0 = A0*B0 + beta * (A1*B1)
C1 = (A0 + A1) * (B0 + B1) - A0*B0 - A1*B1
```

Implementation in three base-field GEMMs:

```
M0 = gemm(A0, B0)           // base-field GEMM, m x n
M1 = gemm(A1, B1)           // base-field GEMM, m x n
M2 = gemm(A0 + A1, B0 + B1) // base-field GEMM on sum matrices, m x n

C0 = M0 + beta * M1          // m*n base-field add + m*n scalar mul by beta
C1 = M2 - M0 - M1            // 2 * m*n base-field subtractions
```

The `beta * M1` step multiplies every element of M1 by the scalar `C::NON_RESIDUE`. Since beta is a single base-field scalar, this is `m*n` calls to `C::mul_by_non_residue`, not a full GEMM. When `C::mul_by_non_residue` has a fast override (e.g., negation for `beta = -1`), this cost is negligible.

Matrix pre/post processing per GEMM call:

- Extracting A0, A1 from `FieldMatrix<QuadraticExt<C>>`: iterate elements, copy `.c0()` and `.c1()` — O(m*k) base-field copies.
- Building sum matrices A0 + A1, B0 + B1: O(m*k + k*n) base-field additions.
- Assembling output from C0, C1 into `FieldMatrix<QuadraticExt<C>>`: O(m*n) copy.

Total base-field GEMM cost: 3 GEMMs of size m*k*n (vs 4 GEMMs' worth in a naive 4-product schoolbook implementation). The overhead of extraction + sum-matrix construction is O(m*k + k*n) and is dominated by the GEMM cost for any non-trivially sized matrix.

### Reference for matrix Karatsuba

Burgisser, Clausen, Shokrollahi, "Algebraic Complexity Theory", Springer, 1997, Chapter 14 discusses bilinear algorithms and their matrix-level lifts. Sedoglavic, "A polynomial time algorithm to reduce the number of variables in a multilinear system...", arXiv:1108.4841 discusses multi-linear lifting.

For the algorithmic correctness of the matrix-level lift, it follows directly from the distributivity of matrix multiplication over addition: the Karatsuba identity `c1 = (a0+a1)(b0+b1) - a0*b0 - a1*b1` holds at any commutative-ring level, and matrix rings are rings (though not commutative). Since the base-field components A0, A1, B0, B1 commute with each other as ring elements (they are matrices over a commutative field), the substitution is valid.

---

## Decomposition -- Cubic

### Scalar 6-mul formula (existing, for reference)

For elements `A = a0 + a1*v + a2*v^2`, `B = b0 + b1*v + b2*v^2` with v^3 = beta:

```text
v0 = a0*b0,  v1 = a1*b1,  v2 = a2*b2
x  = (a1+a2)*(b1+b2) - v1 - v2     // a1*b2 + a2*b1
y  = (a0+a1)*(b0+b1) - v0 - v1     // a0*b1 + a1*b0
z  = (a0+a2)*(b0+b2) - v0 + v1 - v2 // a0*b2 + a1*b1 + a2*b0  (note: +v1 not -v1)
c0 = v0 + beta*x
c1 = y  + beta*v2
c2 = z
```

This is 6 base-field multiplications (v0, v1, v2, x-cross, y-cross, z-cross). Source: `crates/gf2-core/src/gfpn/cubic.rs`, lines 1-54 doc header and the multiplication implementation (see `batch.rs` line 963-1008 for the SoA version). Same Devegili et al. reference.

### Approach selection: recursive Karatsuba over Toom-Cook

Two options exist for lifting to matrices:

**Option A: Toom-Cook 3** evaluates the polynomial A(x) at 5 points, multiplies, and interpolates. The interpolation adds rational arithmetic overhead (divisions by small integers like 2, 3, 6) that complicates the base-field GEMM integration unless the characteristic is large enough to guarantee invertibility. This is more fragile.

**Option B: Recursive Karatsuba / Winograd cubic** directly maps the 6-mul formula. This lifts cleanly because the formula uses only addition, subtraction, and scalar multiply-by-beta — all of which work at the matrix level by the same distributive argument as the quadratic case.

**Decision: this design chooses recursive Karatsuba (Option B)**. The 6-mul formula is already proven correct in the production scalar and batch code; lifting it is mechanical and the result requires no additional infrastructure (no characteristic-dependent division checks). Toom-Cook would be preferable at higher degrees but the complexity/risk ratio for cubic is not worth it.

Reference: Toom, A.L., "On the Complexity of a Scheme of Functional Elements Realizing the Multiplication of Integers", Soviet Mathematics Doklady, 3:714-716, 1963. Cook, S.A., "On the Minimum Computation Time of Functions", Harvard PhD thesis, 1966. Bodrato, M. and Zanoni, A., "Integer and Polynomial Multiplication: Towards Optimal Toom-Cook Matrices", ISSAC 2007. For the Winograd reduction specifically: Winograd, S., "On multiplication of 2x2 matrices", Linear Algebra and its Applications 4(4):381-388, 1971.

### Matrix-level recursive Karatsuba lift (cubic, 6 GEMMs)

Let A = A0 + A1*v + A2*v^2 and B = B0 + B1*v + B2*v^2, with v^3 = beta.

```
// Three diagonal products:
M0 = gemm(A0, B0)           // base-field GEMM
M1 = gemm(A1, B1)           // base-field GEMM
M2 = gemm(A2, B2)           // base-field GEMM

// Three cross-term products:
Mx = gemm(A1 + A2, B1 + B2) // base-field GEMM
My = gemm(A0 + A1, B0 + B1) // base-field GEMM
Mz = gemm(A0 + A2, B0 + B2) // base-field GEMM

// Recover cross terms:
X = Mx - M1 - M2             // a1*B2 + a2*B1 lift: m*n sub x2
Y = My - M0 - M1             // a0*B1 + a1*B0 lift: m*n sub x2
Z = Mz - M0 + M1 - M2        // a0*B2 + a1*B1 + a2*B0 lift: m*n sub x2, add x1

// Assemble output:
C0 = M0 + beta * X           // beta is base-field scalar; m*n mul_by_non_residue + add
C1 = Y  + beta * M2          // m*n mul_by_non_residue + add
C2 = Z
```

Total cost: 6 base-field GEMMs + O(m*n + m*k + k*n) base-field add/sub operations for sum-matrix construction and output assembly.

Schoolbook (naive) cost for cubic would be 9 base-field GEMMs (a0*b0 through a2*b2 without Karatsuba). The 6-GEMM formula gives a 1.5x reduction in the GEMM-count, same as the scalar formula reduces 9 scalar multiplications to 6.

### Note on z formula sign

The z-cross computation differs from the x and y by having `+v1` instead of `-v1`. This is correct per the Devegili formula and the existing production code in `batch.rs` lines 995-1000. The matrix lift faithfully reproduces the same sign structure.

---

## API Surface

### Proposed design: free functions in `gfpn::matrix_karatsuba`

We propose new free functions (not a trait method on `FieldMatrix`) because:

1. Adding inherent methods on `FieldMatrix<QuadraticExt<C>>` requires the compiler to unify `F = QuadraticExt<C>` at the impl site, which requires `where C: ExtConfig` — this is expressible but adds syntactic noise at every call site.
2. Free functions in a dedicated sub-module (`crates/gf2-core/src/gfpn/matrix_karatsuba.rs`) keep the extension-field specialization self-contained and clearly visible.
3. A `Specialised` trait dispatch in `field::matrix::gemm` (option c) would require a new marker trait threaded through the existing dispatch table in `matrix.rs`; this is more invasive and harder to review.
4. Free functions can later be wired into the `field::matrix::gemm` dispatch as a pre-check (child issue 3 below) without changing their signature.

The proposed public API (signatures, not implementations):

```rust
// crates/gf2-core/src/gfpn/matrix_karatsuba.rs

/// Karatsuba matrix multiplication for quadratic extensions.
///
/// Computes `A * B` where `A` and `B` are `FieldMatrix<QuadraticExt<C>>`.
/// Decomposes into 3 base-field GEMMs on `FieldMatrix<C::BaseField>`.
///
/// # Arguments
/// * `a` — left operand, shape m x k.
/// * `b` — right operand, shape k x n.
///
/// # Returns
/// `FieldMatrix<QuadraticExt<C>>` of shape m x n.
///
/// # Panics
/// Panics if inner dimensions do not match.
pub fn quadratic_gemm<C: ExtConfig>(
    a: &FieldMatrix<QuadraticExt<C>>,
    b: &FieldMatrix<QuadraticExt<C>>,
) -> FieldMatrix<QuadraticExt<C>>
where
    C::BaseField: ConstField;

/// Karatsuba-3 matrix multiplication for cubic extensions.
///
/// Computes `A * B` where `A` and `B` are `FieldMatrix<CubicExt<C>>`.
/// Decomposes into 6 base-field GEMMs on `FieldMatrix<C::BaseField>`.
///
/// # Arguments
/// * `a` — left operand, shape m x k.
/// * `b` — right operand, shape k x n.
///
/// # Returns
/// `FieldMatrix<CubicExt<C>>` of shape m x n.
///
/// # Panics
/// Panics if inner dimensions do not match.
pub fn cubic_gemm<C: ExtConfig>(
    a: &FieldMatrix<CubicExt<C>>,
    b: &FieldMatrix<CubicExt<C>>,
) -> FieldMatrix<CubicExt<C>>
where
    C::BaseField: ConstField;
```

Both functions are additive — they do not modify any existing function signatures. Callers wishing to use the fast path call `quadratic_gemm` or `cubic_gemm` explicitly. Child issue 3 (integration) wires these into the `field::matrix::gemm` dispatch so that callers of the `*` operator or `field::matrix::gemm` get the fast path automatically.

The module is exported as `pub mod matrix_karatsuba` in `crates/gf2-core/src/gfpn/mod.rs`. No changes to `crates/gf2-core/src/field/matrix.rs` in this phase.

### Helper functions (crate-internal)

Within the implementation:

```rust
// Extract A0, A1 component matrices from FieldMatrix<QuadraticExt<C>>
fn split_quadratic<C: ExtConfig>(
    a: &FieldMatrix<QuadraticExt<C>>,
) -> (FieldMatrix<C::BaseField>, FieldMatrix<C::BaseField>);

// Assemble FieldMatrix<QuadraticExt<C>> from C0 and C1 component matrices
fn assemble_quadratic<C: ExtConfig>(
    c0: FieldMatrix<C::BaseField>,
    c1: FieldMatrix<C::BaseField>,
) -> FieldMatrix<QuadraticExt<C>>;

// Same for cubic:
fn split_cubic<C: ExtConfig>(
    a: &FieldMatrix<CubicExt<C>>,
) -> (FieldMatrix<C::BaseField>, FieldMatrix<C::BaseField>, FieldMatrix<C::BaseField>);

fn assemble_cubic<C: ExtConfig>(
    c0: FieldMatrix<C::BaseField>,
    c1: FieldMatrix<C::BaseField>,
    c2: FieldMatrix<C::BaseField>,
) -> FieldMatrix<CubicExt<C>>;
```

These helpers iterate over elements using the existing `.c0()`, `.c1()`, `.c2()` accessors (already public on `QuadraticExt` and `CubicExt`) and call `QuadraticExt::new` / `CubicExt::new` for assembly. No new public API is needed on the element types.

---

## Routing Through Existing Base-Field Acceleration

### For `QuadraticExt<Fp<65537>>`

The 3 sub-GEMMs in `quadratic_gemm` are:

```rust
let m0 = field::matrix::gemm(&a0, &b0);  // FieldMatrix<Fp<65537>> * FieldMatrix<Fp<65537>>
let m1 = field::matrix::gemm(&a1, &b1);  // same
let m2 = field::matrix::gemm(&a0_plus_a1, &b0_plus_b1);  // same
```

Each of these hits the existing `field::matrix::gemm` dispatch at line 2642 of `matrix.rs`:

```rust
if F::try_simd_gemm_classical(...) { return out; }
```

For `F = Fp<65537>`, `try_simd_gemm_classical` dispatches into the dedicated Fermat-prime AVX2 kernel in `crates/gf2-kernels-simd/src/fp65537.rs`. This is **automatic** — no changes needed to the base-field path. The 3 sub-GEMMs each use the full fast path.

The acceleration inheritance is verified by tracing the dispatch in `crates/gf2-core/src/field/matrix.rs` line 2642: `F::try_simd_gemm_classical` is defined on `FiniteField` with a default returning `false`, and `Fp<65537>` overrides it via the SIMD dispatch in `gfp/simd_ops.rs` (routing to `crates/gf2-kernels-simd/src/fp65537.rs`).

### For `CubicExt<Fp<65537>>`

The 6 sub-GEMMs in `cubic_gemm` are all `FieldMatrix<Fp<65537>>` products, so they likewise inherit the Fp<65537> fast path automatically.

### For other primes

`QuadraticExt<Fp<P>>` with `P <= 251` will route its 3 sub-GEMMs through the small-prime AVX2 Candidate C path (`try_simd_gemm_classical` for small primes). `QuadraticExt<Fp<P>>` with `P` in the medium-prime range hits `try_fp_simd_dot_packed_u16`. `QuadraticExt<Fp<P>>` with other `P` falls back to the scalar `dot_product_slices` path, but still benefits from the 3-GEMM vs 4-schoolbook reduction.

---

## Correctness Tests

Tests are **specified here** (not written yet -- implementation children write them).

### Round-trip equality tests

Per SC#1.3: for each of `QuadraticExt<Fp<P>>` and `CubicExt<Fp<P>>` at n in {16, 64, 256}, verify that `quadratic_gemm(A, B) == field::matrix::gemm(A, B)` exactly (element-by-element equality).

Base primes to cover:

| Base prime | Rationale |
|------------|-----------|
| `Fp<7>`   | Small prime, exhaustive-friendly |
| `Fp<251>` | Byte-prime SIMD path for small-prime sub-GEMMs |
| `Fp<65537>` | Dedicated fast path (Fermat prime) |

Matrix sizes: n in {16, 64, 256} for square matrices. Also include a non-square case (e.g., 16x64 * 64x32) to verify the m != k != n path.

Test structure: use `proptest!` macros to generate random matrices, not `#[test]` with hard-coded matrices. For small primes the proptest strategy samples elements as (u64 % P, u64 % P) tuples.

### Field axiom tests for the decomposed product

For small n (e.g., n=4 for exhaustive, n=16 for proptest):
- associativity: `quadratic_gemm(quadratic_gemm(A, B), C) == quadratic_gemm(A, quadratic_gemm(B, C))`
- distributivity: `quadratic_gemm(A, B + C) == quadratic_gemm(A, B) + quadratic_gemm(A, C)`
- identity: `quadratic_gemm(I, A) == A` where I is the extension-field identity matrix

These tests are property-based using `proptest`.

### Edge cases

- n=1 (scalar product, degenerates to element multiplication)
- m=0 or n=0 (empty matrix)
- k=0 (zero inner dimension, result is all-zeros)

---

## Benchmark Cells

Per SC#1.4:

| Field                    | n    | Notes |
|--------------------------|------|-------|
| `QuadraticExt<Fp<65537>>` | 64  | Fast-path sub-GEMMs |
| `QuadraticExt<Fp<65537>>` | 256 | Primary benchmark cell |
| `QuadraticExt<Fp<65537>>` | 1024 | Large-n throughput |
| `CubicExt<Fp<65537>>`    | 64  | Fast-path, 6 sub-GEMMs |
| `CubicExt<Fp<65537>>`    | 256 | Primary benchmark cell |
| `CubicExt<Fp<65537>>`    | 1024 | Large-n throughput |
| `QuadraticExt<Fp<251>>`  | 256 | Small-prime sub-GEMMs |
| `QuadraticExt<Fp<251>>`  | 1024 | Small-prime large-n |

### Baseline

No extension-field matrix GEMM baseline exists in the current repository. The benchmark cells use **generic matmul on-host as a relative-speedup baseline**: measure `field::matrix::gemm` (the existing path) and `quadratic_gemm` (the new path) in the same benchmark session on the reference host (AMD Ryzen 9 5900X). Report as `speedup = new_Gops / old_Gops`.

Expected speedup: for large n, the Karatsuba path should approach a 4/3 = 1.33x improvement for quadratic (3 GEMMs vs 4-equivalent) and 6/9 = 1.5x for cubic, amplified by the base-field SIMD fast path engaging for the now-unified base-field sub-GEMMs. Actual numbers will be measured at implementation time.

The benchmark should be added to `crates/gf2-core/benches/` as `ext_field_matrix_gemm.rs`, following the criterion framework used by existing `gf2-core` benches. The bench is part of child issue 1 (quadratic) and child issue 2 (cubic).

---

## Non-Regression Contract

Per SC#1.5: no regression on existing extension-field paths used by `gf2-coding` or `gf2-algebra`.

### Survey of QuadraticExt and CubicExt usage outside gf2-core

Verified by `rg -n 'QuadraticExt\|CubicExt' crates/gf2-coding/src/ crates/gf2-algebra/src/`:

| File | Line | Usage type | Matrix GEMM affected? |
|------|------|-----------|----------------------|
| `crates/gf2-algebra/src/permanent/ryser.rs:76` | Comment mentioning QuadraticExt and CubicExt as examples of FiniteField | Documentation only | No |

No production code in `gf2-coding` or `gf2-algebra` directly uses `QuadraticExt` or `CubicExt` in a matrix GEMM call. The sole reference in `gf2-algebra/src/permanent/ryser.rs` is a doc comment listing these types as examples of the `FiniteField` trait, not a direct use.

### What the non-regression contract covers

1. **Scalar arithmetic (not affected):** `QuadraticExt::mul`, `CubicExt::mul`, and all other scalar operations are unchanged. The new `quadratic_gemm` and `cubic_gemm` functions call the existing scalar arithmetic indirectly via `field::matrix::gemm` for the sub-GEMMs. No changes to `quadratic.rs`, `cubic.rs`, `ext_config.rs`, or `batch.rs` are required.

2. **`BatchExtField` (not affected):** The batch element-wise Karatsuba in `batch.rs` (`batch_mul_quadratic`, `batch_mul_cubic`) operates on independent element-wise products, not matrix-level GEMMs. It is a different operation. The new matrix-level functions do not touch `batch.rs`.

3. **Existing `FieldMatrix<QuadraticExt<_>>` users (get speedup automatically in child 3):** After child issue 3 wires the new functions into `field::matrix::gemm`'s dispatch, callers using `A * B` on extension-field matrices will automatically get the fast path. The result is bit-identical to the old path (guaranteed by the correctness tests in child 1 and 2). Callers that currently use `field::matrix::gemm` directly will also benefit.

4. **Gate requirement:** child issues 1, 2, and 3 must all pass `cargo nextest run -p gf2-core --release --profile ci` without regression. The CI test suite already covers scalar extension-field arithmetic and will catch any accidental breakage.

---

## Implementation Child Issues

Listed in dependency order. Each child is a separate JIT issue.

### Child 1 — Quadratic Karatsuba matrix-GEMM implementation

**Scope:** Implement `gfpn::matrix_karatsuba::quadratic_gemm` and the `split_quadratic` / `assemble_quadratic` helpers. Write correctness tests (round-trip vs generic GEMM via `proptest`) for `QuadraticExt<Fp<P>>` at n in {16, 64, 256} over `Fp<7>`, `Fp<251>`, `Fp<65537>`. Write the benchmark cells for `QuadraticExt<Fp<65537>>` and `QuadraticExt<Fp<251>>` at n in {64, 256, 1024}.

**Dependencies:** none (design doc is the only prerequisite).

**Expected gates:** `cargo-ci`, `code-review`, `doc-review`.

**Success criteria:**
- [hard] `quadratic_gemm(A, B) == field::matrix::gemm(A, B)` (bit-exact) for all proptest cases at n in {16, 64, 256} over `Fp<7>`, `Fp<251>`, `Fp<65537>`.
- [hard] No regression on existing `gf2-core` test suite.
- [hard] Benchmark result recorded (speedup vs generic, on reference host).
- [hard] No `unsafe` blocks.
- [hard] No production code in `gf2-coding` or `gf2-algebra` broken.

### Child 2 — Cubic Karatsuba-3 matrix-GEMM implementation

**Scope:** Implement `gfpn::matrix_karatsuba::cubic_gemm` and `split_cubic` / `assemble_cubic`. Correctness tests for `CubicExt<Fp<P>>` at n in {16, 64, 256} over `Fp<7>`, `Fp<251>`, `Fp<65537>`. Benchmark cells for `CubicExt<Fp<65537>>` at n in {64, 256, 1024}.

**Dependencies:** Child 1 (for the module/helper pattern established there; logically parallel but Child 1 should land first to establish the module structure).

**Expected gates:** `cargo-ci`, `code-review`, `doc-review`.

**Success criteria:**
- [hard] `cubic_gemm(A, B) == field::matrix::gemm(A, B)` (bit-exact) for all proptest cases at n in {16, 64, 256} over `Fp<7>`, `Fp<251>`, `Fp<65537>`.
- [hard] No regression on existing `gf2-core` test suite.
- [hard] Benchmark result recorded.
- [hard] No `unsafe` blocks.
- [hard] 6-GEMM formula matches the formula in this design document verbatim.

### Child 3 — Integration: automatic dispatch via `field::matrix::gemm`

**Scope:** Wire `quadratic_gemm` and `cubic_gemm` into the `field::matrix::gemm` dispatch so that callers using `*` or `field::matrix::gemm` on extension-field matrices automatically use the fast path. The integration mechanism is a new pre-check in `gemm`:

```rust
// In field/matrix.rs::gemm, before the simd_gemm_classical attempt:
if let Some(result) = F::try_karatsuba_extension_gemm(a, b_t, ...) {
    return result;
}
```

This requires a new `try_karatsuba_extension_gemm` hook on `FiniteField` with a default returning `None`, overridden by `QuadraticExt<C>` and `CubicExt<C>`. Alternatively, the implementation can use a simpler approach: add a new module-level check before the existing dispatch that matches on `TypeId` or uses a separate blanket impl. The specific mechanism is left to the implementation; the observable contract is that `A * B` on extension-field matrices produces the same result as `quadratic_gemm(A, B)`.

**Dependencies:** Child 1 and Child 2 (both must be landed before dispatch is wired).

**Expected gates:** `cargo-ci`, `code-review`, `doc-review`.

**Success criteria:**
- [hard] `A * B` (operator) on `FieldMatrix<QuadraticExt<C>>` produces the same result as `quadratic_gemm(&A, &B)`.
- [hard] `A * B` on `FieldMatrix<CubicExt<C>>` produces the same result as `cubic_gemm(&A, &B)`.
- [hard] No regression on existing tests for other field types (Fp<P>, Gf2mElement, etc.).
- [hard] The dispatch does not add overhead for non-extension-field types.

### Child 4 (optional) — Bench harness for extension-field matrix GEMM cells

**Scope:** Add a standalone bench script (mirroring `dev/bench_results/run_41096af5_post_wire_in_bench.sh`) that runs the extension-field benchmark cells with pinned CCX methodology and records results as CSV + evidence doc. This is only needed if the bench in children 1/2 is not sufficient for the Phase 5 scorecard.

**Dependencies:** Child 3.

**Expected gates:** `doc-review` (evidence doc), `code-review` (bench script).

---

## Open Questions

### Q1: Should the dispatch in Child 3 use a new `FiniteField` hook or a type-based check?

**Default:** use a new default-`None` hook on `FiniteField`, called `try_karatsuba_gemm`. This mirrors the existing `try_simd_gemm_classical` pattern and avoids `TypeId` comparisons. The downside is it adds one more method to an already large trait. An alternative is a separate module-level function that checks a sealed trait implementation at compile time.

**Decision needed by:** start of Child 3 dispatch. Not blocking for Children 1 and 2.

### Q2: Should `split_quadratic` copy into a new `FieldMatrix<C::BaseField>` or return views?

**Default:** copy into a new `FieldMatrix<C::BaseField>`. Views would require a `StrideMatrix` or similar abstraction (the coefficient stride is 2x for a QuadraticExt row). A copy is O(m*k) and is dwarfed by the O(m*k*n) GEMM; for large n the copy is negligible.

An alternative is to extend `MatrixLike` to support strided reads of sub-components. This is a more invasive change and is not needed for correctness.

**Decision needed by:** start of Child 1.

### Q3: Should the Karatsuba functions be feature-gated?

**Default:** no feature gate. The functions are pure Rust, contain no SIMD intrinsics, and the speedup comes entirely from reusing the base-field GEMM dispatch (which is already feature-gated internally). Adding a feature gate would complicate the API surface without benefit.

**Decision needed by:** start of Child 1.

### Q4: Crossover threshold -- at what n does Karatsuba GEMM beat generic GEMM?

For very small n (n=1, n=2), the overhead of splitting and assembling matrices plus three GEMM calls may exceed the savings. The scalar Karatsuba already applies at n=1 (element level), so the matrix-level Karatsuba should never be slower if n >= some threshold.

**Default assumption:** the crossover is at n <= 4 or smaller (i.e., the matrix-level decomposition wins for any n >= 4). This will be verified empirically in Child 1. If the crossover is larger, the dispatch in Child 3 should add a size guard:

```rust
if rows >= KARATSUBA_THRESHOLD && cols >= KARATSUBA_THRESHOLD {
    return karatsuba_path(...);
}
```

**Decision needed by:** end of Child 1 (measurement determines whether a threshold guard is needed).

---

## Source Index

| File | Lines | Role in this design |
|------|-------|---------------------|
| `crates/gf2-core/src/gfpn/mod.rs` | 1-64 | Module map; `matrix_karatsuba` will be added here |
| `crates/gf2-core/src/gfpn/ext_config.rs` | 62-110 | `ExtConfig` trait: `BaseField`, `NON_RESIDUE`, `mul_by_non_residue` |
| `crates/gf2-core/src/gfpn/quadratic.rs` | 229-350, 451-467 | `QuadraticExt<C>` struct, accessors (c0, c1), Karatsuba mul |
| `crates/gf2-core/src/gfpn/cubic.rs` | 1-54 | `CubicExt<C>` struct, 6-mul Karatsuba formula (doc), accessors |
| `crates/gf2-core/src/gfpn/batch.rs` | 963-1008, 1019-1063 | `batch_cubic_karatsuba`, `batch_karatsuba`, `scalar_karatsuba` -- SoA element-wise versions of the same formulas used here at matrix level |
| `crates/gf2-core/src/field/matrix.rs` | 2562-2711 | `gemm` dispatch: `try_simd_gemm_classical`, `try_gf2m_u64_batch_dot_product`, `try_fp_simd_dot_packed_u16`, `dot_product_slices` fallback |
| `crates/gf2-algebra/src/permanent/ryser.rs` | 76 | Only use of `QuadraticExt`/`CubicExt` outside `gf2-core` -- doc comment, not code |
| `dev/active/615db3b9-finite-field-la-sota-plan.md` | Phase 4 section | Authoritative plan reference for this work |

# fflas-ffpack: Analysis and Applicability to gf2

> Reference analysis for epic e095a100 — "Implement general Galois field GF(p^m) arithmetic"
>
> Source: [linbox-team/fflas-ffpack](https://github.com/linbox-team/fflas-ffpack) (C++, LGPL-2.1)
>
> This document analyzes fflas-ffpack's architecture to identify design patterns applicable
> to our finite field linear algebra implementation and to establish it as the primary
> performance benchmark target.

## 1. What fflas-ffpack Is

fflas-ffpack is a C++ template library providing **exact linear algebra over finite fields**
by leveraging numerical BLAS (Basic Linear Algebra Subprograms). Its core innovation:
route finite field computations through optimized floating-point BLAS routines (OpenBLAS,
MKL) and recover exact results via modular reduction, achieving near-BLAS throughput for
exact arithmetic.

Two layers mirror the classical numerical stack:
- **FFLAS** (Finite Field Linear Algebra Subprograms) — BLAS levels 1–3: vector ops,
  matrix-vector, matrix-matrix multiply
- **FFPACK** (Finite Field PACKage) — LAPACK-level: LU decomposition, rank, determinant,
  characteristic polynomial, minimal polynomial, echelon forms

Dependencies: Givaro (field implementations), a BLAS library (OpenBLAS/ATLAS/MKL), GMP.

## 2. Field Abstraction: Three-Level Classification

fflas-ffpack uses C++ template traits to classify fields at compile time and route
operations to optimal backends. Three orthogonal trait axes:

### 2.1 ElementTraits — what the element looks like in memory

| Tag | Types | Notes |
|-----|-------|-------|
| `MachineFloatTag` | `float`, `double` | Native FPU, BLAS-compatible |
| `MachineIntTag` | `int8_t` through `int64_t` | Integer SIMD possible |
| `FixedPrecIntTag` | `RecInt::rint<K>` | Multi-word fixed-precision |
| `ArbitraryPrecIntTag` | `Givaro::Integer` (GMP) | Arbitrary precision |
| `RNSElementTag` | `rns_double_elt` | Residue Number System tuple |

### 2.2 FieldTraits — what kind of field it is

| Tag | Examples |
|-----|----------|
| `ModularTag` | `Modular<T>`, `ModularBalanced<T>` — prime fields with element type T |
| `UnparametricTag` | `ZRing<T>` — rings without modular reduction |

### 2.3 ModeTraits — how to compute over this field

This is the key dispatch mechanism. Given a field type, ModeTraits selects the computation
strategy:

| Mode | Strategy | When used |
|------|----------|-----------|
| `DefaultTag` | Standard field operations, no shortcuts | Fallback |
| `DelayedTag` | Accumulate operations, reduce only when overflow imminent | `Modular<int64_t>` |
| `LazyTag` | Like Delayed, but inputs may also be unreduced | Internal recursion |
| `ConvertTo<MachineFloatTag>` | Convert elements to float/double, use BLAS, convert back | Small primes (p < 2^26) |
| `ConvertTo<RNSElementTag>` | Convert to RNS representation, use BLAS per component | Large primes (> 2^64) |

**Key insight for gf2**: This three-axis classification maps naturally to our planned
`FieldBackend` trait. The element representation → Rust type system, field category →
`FiniteField` trait hierarchy, computation mode → `FieldBackend` dispatch.

## 3. The BLAS Integration Pipeline

### 3.1 Small prime fields → float/double BLAS

For `Modular<int32_t>` with small p:

1. **Convert**: `fconvert()` — field elements (integers) → `float` or `double` array
2. **Reduce**: `freduce()` — ensure values are in [0, p) in the float domain
3. **Compute**: Call CBLAS `dgemm`/`sgemm` — exact because intermediate results fit in
   mantissa (52 bits for double, 23 for float)
4. **Convert back**: `finit()` — float results → field elements with modular reduction

Crossover: `DOUBLE_TO_FLOAT_CROSSOVER = 800`. Fields with p < 800 use float, larger use
double. The bound ensures that the accumulated dot product `k * (p-1)^2` fits in the
mantissa, guaranteeing exact results.

### 3.2 Large prime fields → RNS + BLAS

For primes exceeding machine word size (`Modular<Integer>`):

1. **Decompose**: Large integer X → tuple $(r_1, r_2, \ldots, r_n)$ where $r_i = X \bmod m_i$
   and each $m_i$ is a small double-compatible prime (~50 bits)
2. **The CRT basis change is itself a matrix multiplication**: Integer→RNS uses BLAS to
   multiply a Kronecker-chunked representation by precomputed CRT constants
3. **Compute**: Arithmetic in each RNS channel is independent — add/multiply componentwise
   in `Modular<double>`, using BLAS for matrix operations
4. **Reconstruct**: RNS→Integer via inverse CRT, again using BLAS for the basis change

This is the architecture referenced by our research task 0a7e2555 (RNS representation).

### 3.3 Medium integer fields → direct or delayed

For `Modular<int64_t>`, fflas-ffpack uses an internal `igemm` (integer GEMM) kernel or
delayed reduction: accumulate up to `kmax` multiply-add operations in wider storage before
reducing. No float conversion needed.

## 4. Delayed Reduction with Bounds Tracking

The `MMHelper` (Matrix Multiplication Helper) template is fflas-ffpack's mechanism for
minimizing modular reductions. It tracks:

- **Input bounds**: `(Amin, Amax)` and `(Bmin, Bmax)` — the range of actual values in
  input matrices (may be tighter than [0, p-1] if recently reduced)
- **Output bounds**: `(Outmin, Outmax)` — computed from inputs after accumulation
- **Max storable value**: How large an integer can be represented exactly in the
  accumulator type

The critical computation:

$$k_{\max} = \frac{\text{MaxStorableValue} - |\beta \cdot C_{\max}|}{A_{\max} \cdot B_{\max}}$$

where $k_{\max}$ is the number of multiply-accumulate iterations possible before overflow
requires a reduction pass.

**Application to gf2**: This maps directly to our planned `Wide` associated type on
`FiniteField`. For `Fp<P>` with `Wide = u128`:

$$k_{\max} = \frac{2^{128} - 1}{(P - 1)^2}$$

For GF(2^m) with polynomial multiplication, the bound depends on the number of XOR
accumulations before the polynomial degree exceeds the `Wide` register.

## 5. Algorithmic Optimizations

### 5.1 Winograd matrix multiplication

fflas-ffpack uses Strassen-Winograd for large matrices: 7 recursive multiplications instead
of 8, reducing asymptotic complexity from $O(n^3)$ to $O(n^{2.807})$. Each sub-problem
carries conservative bounds inherited from the parent, tracked by `MMHelper`.

The `WINOTHRESHOLD` constant determines the crossover point from classical to Winograd.

### 5.2 Blocked algorithms

FFPACK factorizations (LU, echelon forms) use block algorithms that reduce to FFLAS level-3
calls, maximizing cache locality and BLAS utilization.

## 6. Core Operations

### FFLAS (BLAS-equivalent)

| Level | Operations |
|-------|------------|
| 1 (Vector) | `fadd`, `fsub`, `fscal`, `faxpy`, `fdot`, `fcopy` |
| 2 (Matrix-Vector) | `fgemv`, `fger`, `ftrsv` |
| 3 (Matrix-Matrix) | `fgemm`, `ftrsm`, `ftrmm`, `fsyrk`, `fsyr2k` |

### FFPACK (LAPACK-equivalent)

- LU decomposition (LQUP, PLUQ)
- Rank, determinant, nullspace
- Row/column echelon forms
- Characteristic and minimal polynomial
- Matrix inversion, linear system solving

### Sparse support

CSR, COO, ELL, SELL-C-σ formats with sparse matrix-vector and matrix-matrix products.

## 7. Applicability to gf2

### 7.1 Not suitable as a direct dependency

| Reason | Detail |
|--------|--------|
| **Language barrier** | C++ template metaprogramming; FFI would require instantiating every `<Field, Algorithm, Mode>` combination |
| **Givaro dependency** | Pulls in Givaro + GMP, with autotools build system |
| **Architectural mismatch** | Core trick is float-BLAS routing; in Rust we build SIMD kernels directly |
| **No GF(2^m) specialization** | Focused on prime fields; our M4RM and PCLMULQDQ kernels are already more specialized |

### 7.2 Design patterns to adopt

| Pattern | fflas-ffpack | gf2 mapping |
|---------|-------------|-------------|
| **Wide accumulator** | `MMHelper` bounds tracking | `FiniteField::Wide` associated type with `reduce_wide()` and `max_unreduced_additions()` |
| **Computation mode dispatch** | `ModeTraits<Field>::value` | `FieldBackend` trait dispatch in `gf2-kernels-simd` |
| **Delayed reduction** | `DelayedTag` / `LazyTag` | `FieldVec::dot_product_delayed()` accumulating in `Wide` |
| **RNS for large primes** | `rns_double` module | Research task 0a7e2555; reference fflas-ffpack's CRT-via-BLAS approach |
| **Blocked factorizations** | FFPACK block LU → FFLAS fgemm | FieldMatrix Gaussian elimination dispatching to FieldVec dot products |
| **Winograd for large matmul** | `schedule_winograd.inl` | Optional in FieldMatrix matmul, gated by matrix size threshold |

### 7.3 Patterns not applicable

| Pattern | Why not |
|---------|---------|
| **Float-BLAS conversion** | We write SIMD kernels directly (AVX-512 IFMA for GF(p), VPCLMULQDQ for GF(2^m)); no benefit from float detour in Rust |
| **Givaro field model** | We have our own two-tier `FiniteField`/`ConstField` hierarchy |
| **CBLAS bindings** | Unnecessary runtime dependency; our kernels are self-contained |

## 8. Performance Benchmark Targets

fflas-ffpack is the primary benchmark target for finite field linear algebra. Key metrics:

| Operation | Field | Matrix sizes | Target |
|-----------|-------|-------------|--------|
| Matrix multiply | GF(p), p < 2^16 | 64² to 4096² | Within 2× of fflas-ffpack |
| Matrix multiply | GF(2^m), m = 8,16,32 | 64² to 4096² | **Exceed** fflas-ffpack (specialized PCLMULQDQ kernels) |
| Gaussian elimination | GF(p) | 64² to 4096² | Within 2× |
| Matrix inversion | GF(p) | 64² to 1024² | Within 2× |
| Linear solve | GF(p) | 64² to 4096² | Within 2× |

For GF(2), our existing M4RM implementation is already specialized and should be compared
against M4RI (the dedicated GF(2) library) rather than fflas-ffpack.

## 9. Key Takeaways

1. **The `Wide` accumulator pattern is the single most impactful adoption.** It enables
   generic delayed-reduction without field-specific code in matrix/vector algorithms.

2. **Bounds-aware reduction scheduling** (`kmax` computation) should be part of the
   `FieldVec` SIMD kernel, not the matrix layer. The matrix layer calls dot product;
   the dot product internally manages reduction scheduling.

3. **RNS is proven for large primes** but can be deferred until we have concrete use cases.
   When needed, fflas-ffpack's `rns-double` module is the reference implementation.

4. **Winograd is worth implementing** for large matrix multiplication once the basic
   blocked algorithm is working. The bounds-tracking infrastructure needed for delayed
   reduction naturally supports Winograd's sub-problem bound propagation.

5. **Our GF(2^m) path should outperform fflas-ffpack** because we have direct access to
   PCLMULQDQ/VPCLMULQDQ without the float-conversion overhead. This is a key
   differentiator to validate in benchmarks.

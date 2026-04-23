# Dumas–Pernet 2013: Alignment with the `FieldMatrix` epic

> Reference: Jean-Guillaume Dumas & Clément Pernet, *Computational linear algebra over finite fields*, arXiv:1204.3735 (2012); published as a chapter of the *Handbook of Finite Fields* (CRC, 2013).
>
> Scope of this document: distill the paper's algorithmic guidance into concrete constraints on the `FieldMatrix` API, on the epic's story decomposition, and on the per-story success criteria. This is a companion to `fflas_ffpack_analysis.md` — that doc covered the *library*; this doc covers the *paper*.

## 1. Why this paper matters for us

`fflas-ffpack` is the canonical implementation of the patterns Dumas–Pernet describe. The paper is prescriptive, not descriptive: it tells us, for each size regime and each field size, which algorithm is correct and how block-recursion reduces it to `gemm`. Reading the paper gives us the algorithmic *contract* we must meet; `fflas-ffpack` gives us a reference implementation against which to benchmark.

## 2. Algorithmic map — paper section → our stories

| Paper section | Algorithm | Our story |
|---|---|---|
| §1.1 Tiny finite fields | Bitpacking (F₂), bitslicing (F₃, F₅, F₇, …), Kronecker substitution | Out-of-scope for this epic (see §6). `BitMatrix` already covers F₂. |
| §1.2 Word-size prime fields | Delayed-reduction classical gemm | `d48a3cfd` (matmul) — already referenced |
| §1.3 Large finite fields | Extended precision or RNS | Deferred to future epic (research task `0a7e2555` already done) |
| §1.4 Subcubic (Strassen–Winograd) | Recursive 7-mult/15-add with tight bound | `d48a3cfd` — already in scope |
| §2.1 Building blocks | `trsm`, `trmm`, `trtri`, `trtrm` reducing to `gemm` | **NEW: N2** — must be built before `inv/solve` |
| §2.2 PLE decomposition | P · L · Echelon, recursive, handles rank-deficient inputs | `c3f8c1cb` (rewritten) — PLE-first, LU derived |
| §2.3 Echelon forms | RowEchelon & RRE via PLE + `trsm/trtri/trtrm` | `c3f8c1cb` — echelon/RREF are projections of PLE |
| §3 Minimal & characteristic polynomial | Krylov basis, Frobenius normal form | **NEW: N3** |
| §4 Blackbox iterative (Wiedemann, Lanczos) | Sparse solve via matrix-vector iteration | **Deferred** (not in this epic) |
| §5 Sparse reordering | Markowitz, minimum-degree | **Deferred** |
| §6 Hybrid sparse–dense | Mix direct + iterative | **Deferred** |

Stories `ae1d1e88` (inv/solve/det) and `8a90882e` (sparse) remain but now rest on the PLE/`trsm` stack per the paper's prescription.

## 3. Concrete constraints on our API

### 3.1 PLE is the ground truth over finite fields

The paper (§2.2) explicitly argues that over finite fields, the input matrix often has a **non-generic rank profile**. The right decomposition primitive is therefore **PLE** — Permutation · Lower-triangular · Row-Echelon — not PLU.

**API consequence:**
```rust
impl<F: FiniteField> FieldMatrix<F> {
    /// PLE decomposition: returns (P, L, E, rank).
    /// L is m×r, E is r×n in row-echelon form with unit leading coefficients.
    pub fn ple(&self) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize);

    /// LU decomposition derived from PLE; returns None for rank-deficient matrices.
    pub fn lu(&self) -> Option<(Permutation, FieldMatrix<F>, FieldMatrix<F>)>;

    pub fn rank(&self) -> usize;          // reads r from ple()
    pub fn rref(&self) -> FieldMatrix<F>; // alg 2.7, uses PLE + trsm/trtri/trtrm
    pub fn nullspace(&self) -> Vec<FieldVec<F>>;
}
```

LU's reduction to PLE costs nothing algorithmically — when `rank == min(m, n)`, PLE *is* PLU up to the identity on `E`'s leading block.

### 3.2 Triangular primitives are first-class, not hidden

Paper Table 2 (§2.3) shows every factorization's leading constant as a sum of `trsm/trtri/trtrm` leading constants. If we hide these primitives inside `lu()`/`rref()`, we can't share the kernels across algorithms or tune them independently. Expose them:

```rust
// Building blocks (N2). All in-place, zero extra allocation per alg 2.1.
pub fn trsm_upper(a: &FieldMatrix<F>, b: &mut FieldMatrix<F>);   // AX = B, A upper
pub fn trsm_lower(a: &FieldMatrix<F>, b: &mut FieldMatrix<F>);   // AX = B, A lower
pub fn trmm(a: &FieldMatrix<F>, b: &mut FieldMatrix<F>);         // C = A·B, A triangular
pub fn trtri(a: &mut FieldMatrix<F>);                            // A ← A⁻¹, A triangular
pub fn trtrm(l: &mut FieldMatrix<F>, u: &FieldMatrix<F>);        // A = U·L, upper × lower
```

### 3.3 Winograd bound (theorem 4, §1.4) governs delayed-reduction depth

Every intermediate `z` after `l` levels of Strassen–Winograd satisfies
$$|z| \le \left(\frac{1+3^l}{2}\right)^2 \left\lceil \frac{k}{2^l} \right\rceil (p-1)^2.$$

For our `Wide` accumulator and our bounds-aware scheduler (`FiniteField::max_unreduced_additions()`), Winograd's sub-problems must propagate this conservative bound, not the classical gemm bound. `d48a3cfd`'s success criterion *already* references this; we make it explicit in the implementation story.

### 3.4 Echelon forms must share storage with PLE

Per §2.3 (algorithms 2.6, 2.7), RowEchelon and RRE are written *in-place* over the PLE output. We enforce this in the API: `rref()` borrows `self` and returns a new matrix, but internally uses `trsm/trtri` over the PLE output without reallocating.

## 4. Paper-driven additions to the wave plan

The paper justifies three concrete additions to the original 6-story epic:

1. **Block-recursive `trsm/trmm/trtri/trtrm` story (N2)** — without these, `inv` (ae1d1e88) and `solve` will each reinvent the block recursion, duplicating kernels and missing shared tuning opportunities. Must land in Wave 2, alongside `c3f8c1cb` and `d48a3cfd`.

2. **Characteristic and minimal polynomial story (N3)** — §3 gives four algorithms with complexity $O(n^3)$ down to $O(n^\omega \log n)$. We ship the practical deterministic $6n^3$ baseline first, plus the Las-Vegas $O(n^\omega)$ variant over $q > 2n^2$ (theorem 13.4) when our field meets the bound. Builds on `d48a3cfd` (matmul) and `c3f8c1cb` (PLE).

3. **M4RI comparison on GF(2)** — the paper's §1.1 plus `fflas_ffpack_analysis.md` §7 are explicit that `M4RI` is the right GF(2) baseline. Extending `64c88ae4` (terminal benchmark) to include M4RI is a wording change, not a new story.

## 5. Paper-driven constraints on the benchmark story (`64c88ae4`)

- **Baselines split by field**: `fflas-ffpack` for GF(p) and GF(2^m), m ≥ 2; `M4RI` for GF(2).
- **Sizes**: 64², 256², 1024², 4096² as already specified, plus `n² × nᵅ` rectangular cases (α = 0.5, α = 0.3) to stress the Winograd crossover.
- **Randomness regimes**: uniformly random full-rank + specifically rank-deficient matrices (rank = n/2) so PLE's advantage over PLU is measured.
- **Methodology in a container**: reproducibility > raw throughput. We pin gcc, OpenBLAS, fflas-ffpack, M4RI versions in a container image.

## 6. What this paper explicitly does *not* motivate us to add

- **Bitslicing for F₃, F₅, F₇, F₂³** (§1.1). Valuable in principle, but we have no downstream consumer in the current coding-theory stack. Note as a future optimization opportunity.
- **Hybrid sparse-dense direct/iterative solve** (§6). Requires Wiedemann/Lanczos (§4) first; both deferred.
- **RNS for large-prime matrices** (§1.3). Already scoped separately by research task `0a7e2555`; revisit once a concrete large-prime use case exists.

## 7. Concrete cross-reference table for reviewers

When reviewing an implementation PR, the reviewer should check that the cited paper section matches the code:

| PR touches | Paper section | Check |
|---|---|---|
| `matmul.rs` classical path | §1.2 | Delayed reduction with kmax = (Wide_max − |βC|) / (A_max · B_max) |
| `matmul.rs` Winograd path | §1.4, theorem 4 | Bound propagation uses the $(1+3^l)/2$-squared factor |
| `ple.rs` recursion | §2.2, algorithm 2.5 | Splits A column-wise, recurses on A₁ and A₄ |
| `trsm.rs` | §2.1, algorithm 2.1 | Zero extra allocation beyond inputs/outputs |
| `echelon.rs` | §2.3, algorithms 2.6/2.7 | `rref()` uses trtri on L₁ and trmm on L₂ |
| `charpoly.rs` | §3, theorem 13 | Deterministic baseline is $6n^3$ (alg 1) or $O(n^\omega \log n)$ (alg 2) |

This table is the starting point of the `code-review` prompt for each implementation story.

# Sparse benchmark corpus + references — design

> **Issue:** `jit:a3412e15` (Select sparse benchmark corpus and references).
> **Parent story:** `jit:54fd3f0b` (Close sparse FieldMatrix SpMV and SpMM gaps).
> **Authority:** `dev/plans/sota_reference_acceptance_protocol.md` § 2 declares sparse `spmv` and `matmul` (sparse matvec / sparse matmul) in scope; § 3 enumerates the five mandatory acceptance criteria; § 8 lists the exclusion classes.
> **Wave context:** Wave 3 of epic `97bf0879`, dispatched after Wave 2 closed promotion of fflas-ffpack 2.5.0, M4RI 20260122, M4RIE 20250128 (matmul-only), LinBox 1.7.1, NTL 11.6.0, FLINT 3.5.0.

This document specifies the sparse benchmark **corpus** (random / structured / coding-theory matrix classes the post-PPC sparse parity work will time against) and names the accepted **hard references** per `(operation, field-family)` cell — or documents the exclusion of a cell with the protocol § 8 class cited.

The downstream consumer is `jit:47698404` (Re-run sparse post-PPC scorecard), which will *implement* the corpus generators and emit the CSV rows. This document does **not** generate matrices, run benchmarks, or modify `Containerfile` / `run.sh`; those are scoped to `47698404` and follow-ups.

## 1. Acceptance for `a3412e15`'s `[hard]` criteria

Both `[hard]` success criteria are satisfied **in this document**, not deferred to any downstream artefact (per the wave-2 post-mortem: *Hard criteria self-satisfied, not deferred*).

| Issue criterion | Where satisfied |
|---|---|
| The corpus includes random, structured, and coding-theory sparse matrices. | § 3 *Corpus classes* — three subsections (Random / Structured / Coding-theory) each enumerating the matrices to include with shapes, fields, and seed protocol. |
| Accepted references support comparable finite-field semantics. | § 4 *Operations × field matrix* — every `(operation, field-family)` cell carries either a named hard reference (with protocol § 6 *Comparable semantics* cross-equality oracle plan) or `EXCLUDED:<class>:<reason>` per § 8. |

§ 5 records the reproducibility constraints; § 6 lists the exclusion proposals that need user approval before `47698404` dispatches; § 7 links open questions for the lead.

## 2. Scope summary

**In scope.** Sparse-matrix benchmarks corpus and reference selection over the field families already implemented in `gf2-core`:

* GF(2) — owned by `crates/gf2-core/src/sparse.rs` (`SpBitMatrix` CSR + `SpBitMatrixDual` CSR/CSC).
* GF(p) for `p ∈ {7, 251, 65521, 2^31 − 1}` — owned by `crates/gf2-core/src/field/sparse_matrix.rs` (`SparseFieldMatrix<F>` CSR + `SparseFieldMatrixCsc<F>`).
* GF(2^m) for `m ∈ {4, 8, 16}` — same `SparseFieldMatrix<F>` over `Gf2mWide<1, …>`.

Operations covered:

* `spmv` — `y = A·x` (sparse matrix × dense vector).
* `sparse-matmul` — `C = A·B` where `A` is sparse and `B` is sparse (sparse × sparse).
* `sparse×dense` — `C = A·B` where `A` is sparse and `B` is dense (the `matmat` method on `SparseFieldMatrix`).
* `sparse-elim` — sparse Gauss-Jordan / RREF (currently only `SpBitMatrixDual`-style structure-aware elimination on GF(2) is implemented; cross-library coverage lives behind LinBox's `Method::SparseElimination`). Used uniformly across the rest of this document.

**Out of scope.** Dense-only operations (already covered by Wave 2's promotion docs); GPU sparse references (per `97bf0879` parent description); sparse Smith-form / Kalman-style algorithms specific to symbolic algebra (LinBox's `smith-form-sparseelim-*.h` is excluded as "not in epic scope" rather than via a protocol § 8 class).

## 3. Corpus classes

The corpus has **three classes** (random / structured / coding-theory). Every class carries a deterministic seed protocol so the corpus is reproducible across `47698404` and any future re-measurement.

### 3.1 Random — Erdős–Rényi-style sparse matrices

**Definition.** For each `(n, density d)` cell, an `n × n` matrix `A` whose support is sampled by independent `Bernoulli(d)` draws per `(i, j)` cell. Over GF(2), each support cell stores `1`. Over GF(p) and GF(2^m), each support cell carries a uniformly-random non-zero field element drawn from the same SplitMix64 stream **after** the support draw.

**Parameter sweep.**

| Axis | Values |
|---|---|
| `n` (square) | `{1024, 4096, 16384}` |
| `d` (density) | `{1/n, 10/n, log₂(n)/n}` (so the per-row weight `n·d` is `≈ 1`, `≈ 10`, `≈ log₂ n`) |
| Field | GF(2), GF(7), GF(251), GF(65521), GF(2^31 − 1), GF(2^8) (AES `0x1B`), GF(2^16) (`0x002D`) |
| Regime | `uniform` only (rank deficiency on Erdős–Rényi sparse is governed by the density itself; explicit deficient regimes are deferred to T2 because reference libraries' sparse-elim code paths handle them differently) |

Cell count: `3 × 3 × 7 = 63` cells per operation. A run.sh-time `--quick` mode emits only `n ∈ {1024}` × `d ∈ {10/n}` × all 7 fields = 7 cells per operation as a sanity layer; the full sweep is the default.

**Concrete densities for the recommended seed sizes** (rounded for clarity; the harness computes them from `n` and stores the actual density used):

| `n` | `1/n` | `10/n` | `log₂(n)/n` |
|---|---|---|---|
| 1024 | 0.000977 | 0.00977 | 0.00977 (= `10/n` at this size, by coincidence; log₂(1024)=10) |
| 4096 | 0.000244 | 0.00244 | 0.00293 (log₂(4096)=12) |
| 16384 | 0.0000610 | 0.000610 | 0.000854 (log₂(16384)=14) |

Note that `log₂(n)/n` and `10/n` collapse at `n=1024`. The harness still emits both rows so the "expected nnz per row" axis is uniform across `n` — without that, side-by-side rendering for the `log₂(n)/n` regime would have a missing cell at `n=1024`.

**Expected `nnz` per matrix (= `n²·d`).**

| `n` | `1/n` cells | `10/n` cells | `log₂(n)/n` cells |
|---|---|---|---|
| 1024 | ~1024 | ~10 240 | ~10 240 |
| 4096 | ~4096 | ~40 960 | ~49 152 |
| 16384 | ~16 384 | ~163 840 | ~229 376 |

These keep every cell well under 1 GiB of dense-equivalent storage and well under the 30 s `kCellBudgetNs` from protocol § 7.

**Seed protocol.** Reuse the existing `gf2_bench_splitmix64` / `gf2_bench_derive_seed` helpers in `benchmarks/reference/seed_helpers.h` (master `0x6F73AC91D31E4A7C` from `benchmarks/seeds/seed.txt`). For sparse cells the derivation key is `gf2_bench_derive_seed(master, "spmv-er", op_idx, size_idx, regime_idx)` — extending the existing dense convention (which keys on `(operation, size, regime)`) with a `regime_idx` slot. The Rust side already uses an analogous derivation in `crates/gf2-core/benches/sparse_spmv.rs`'s `seed::derive_seed`; the Rust-side code path consumes the new `regime_idx` directly. **The harness MUST call the C/C++ helpers from `benchmarks/reference/seed_helpers.h`** per protocol § 6 *Determinism contract*, so seed drift is structurally impossible across reference-library implementations.

**RNG construction.** Per cell:

1. Compute `seed = gf2_bench_derive_seed(master, "spmv-er-supp", op_idx, size_idx, regime_idx)`.
2. Construct an `n²`-cell Bernoulli mask: walk SplitMix64 from `seed`, draw a `u64` per `(i, j)` (row-major), include if `(draw < threshold)` where `threshold = (d * 2⁶⁴)`. This is a single-pass support sample with `Bernoulli(d)` semantics, exact at f64 precision.
3. Compute `value_seed = gf2_bench_derive_seed(master, "spmv-er-vals", op_idx, size_idx, regime_idx)`.
4. For each cell in the support, draw a uniformly-random non-zero field element from a SplitMix64 chain seeded with `value_seed`. The "non-zero" rejection layer is deterministic given the seed.

Steps 1–2 produce a byte-identical support mask across the gf2-core Rust harness, the LinBox C++ harness, and the fflas-ffpack C++ harness — provided each implementation uses the same SplitMix64 walk order. This is enforced at protocol § 6 smoke time.

### 3.2 Structured — banded, circulant, RCM-permuted

**Definition.** Sparse matrices with structure that real-world sparse algorithms exploit (cache-friendly access patterns, reduced bandwidth, predictable column reuse). The structured class is meant to expose layout-sensitive performance — block-CSR / RCM / prefetch variants in `47698404`'s scorecard depend on having structured inputs to actually win against CSR.

**Matrices to include.**

| ID | Class | Construction | Real-world relevance |
|---|---|---|---|
| `banded-w8` | Banded | Square `n × n` with bandwidth `w = 8`: `A[i][j] ≠ 0` iff `|i − j| ≤ w`. | Stencil discretisations, finite-difference kernels, narrow-band Toeplitz. Cache-line-friendly column access; canonical input for evaluating `matvec_with_prefetch_distance`. |
| `banded-w64` | Banded | As above with `w = 64` (one cache line of `u64` indices on the dev host). | Wider band so block-CSR's `block_rows = 64` boundary gets both intra-block hits and across-block misses. |
| `block-tridiag-32` | Block-tridiagonal | `n × n` with three tridiagonal block bands of `32 × 32` blocks; each block is a random `Bernoulli(0.25)` GF(2) (or scaled-down `Bernoulli(0.1)` over GF(p)). | Discretized PDEs, AR(2) signal models, multi-band LDPC codes. |
| `circulant-w8` | Circulant | Square `n × n` with `A[i][j] = c[(j − i) mod n]` and `c` a fixed-density-`8/n` row. | DFT-related algorithms, filter banks, cyclic codes — every row is a permutation of the same 8 indices, so row-iter cost is uniform. |
| `toeplitz-w16` | Toeplitz | Square `n × n` with `A[i][j] = t[j − i + n − 1]` and `t` a fixed length-`(2n − 1)` density-`16/n` vector. | Convolution operators, polynomial multiplication, signal-processing kernels. Differs from circulant in that wrap-around is replaced by zero padding; row-iter cost varies near edges. |
| `rcm-permuted-er` | RCM-permuted random | Take a `Bernoulli(10/n)` random matrix from § 3.1, apply RCM reorder via `SpBitMatrix::reorder_rcm` (gf2-core path) or an equivalent reference-side reorder, time the matvec on the *reordered* matrix. | Real-world preconditioner workflow: reorder once, multiply many times. The `cbf576d1` evidence (`dev/bench_results/cbf576d1-rcm-sparse-matvec.md`) showed RCM was 0.86× CSR on randomly-generated LDPC-like fixtures; structured RCM-permuted random is a fairer test because the non-RCM baseline starts already cache-hostile. |

**Parameter sweep.**

| Axis | Values |
|---|---|
| `n` | `{1024, 4096, 16384}` (same as random class for direct sweep alignment) |
| Field | GF(2), GF(7), GF(251), GF(65521), GF(2^31 − 1), GF(2^8), GF(2^16) (same set as random class) |
| Regime | `uniform` |

Cell count: `6 (matrices) × 3 (n) × 7 (fields) = 126` cells per operation. The `--quick` mode picks `n=1024` × all 6 matrices × all 7 fields = 42 cells.

**Seed protocol.** Same SplitMix64 chain as § 3.1 with the tag `"spmv-struct"` and a per-matrix sub-tag (`"banded-w8"`, `"banded-w64"`, `"block-tridiag-32"`, `"circulant-w8"`, `"toeplitz-w16"`, `"rcm-permuted-er"`). The structured constructions are deterministic functions of `(seed, matrix-id, n)`; for `rcm-permuted-er`, the underlying random matrix is sampled via the § 3.1 `(seed, n, d=10/n)` cell — so this matrix is doubly determined by both the random-class seed and the RCM permutation, which itself is a deterministic function of the matrix's adjacency graph.

### 3.3 Coding-theory — DVB-T2 / 5G NR / BCH

**Definition.** Sparse matrices drawn directly from `gf2-coding` constructors. These are **the** workload that motivates closing the sparse parity gap: every published modem (DVB-T2, 5G NR) executes thousands of LDPC syndrome-product SpMV per second, so the sparse benchmark corpus is incomplete without their parity-check matrices.

**Matrices to include** (at least three per § 3.3 of the dispatch contract; we include five so the coding-theory class covers both QC-LDPC and dense-generator regimes):

| ID | Construction | `m × n` | nnz | Field | Source / standard |
|---|---|---|---|---|---|
| `dvb-t2-ldpc-r1_2-short` | `LdpcCode::dvb_t2_short(CodeRate::Rate1_2)` parity-check `H` | `8400 × 16200` | ~50 400 (variable; row weight ≈ 6) | GF(2) | ETSI EN 302 755 § 6.1.3, FECFRAME short, rate ½ |
| `dvb-t2-ldpc-r2_3-normal` | `LdpcCode::dvb_t2_normal(CodeRate::Rate2_3)` parity-check `H` | `21600 × 64800` | ~280 000 | GF(2) | ETSI EN 302 755 § 6.1.3, FECFRAME normal, rate ⅔ |
| `nr-5g-bg1-z384` | `LdpcCode::from_quasi_cyclic(QuasiCyclicLdpc::nr_5g(1, 384))` lifted from BG1 (`46×68` base matrix) at `Z = 384` | `17664 × 26112` | ~115 000 (BG1 has ~316 non-zero entries × Z) | GF(2) | 3GPP TS 38.212 § 5.3.2, BG1 |
| `nr-5g-bg2-z208` | `LdpcCode::from_quasi_cyclic(QuasiCyclicLdpc::nr_5g(2, 208))` lifted from BG2 (`42×52` base matrix) at `Z = 208` | `8736 × 10816` | ~40 000 (BG2 has ~197 non-zero entries × Z) | GF(2) | 3GPP TS 38.212 § 5.3.2, BG2 |
| `bch-dvb-t2-normal-t12-G` | DVB-T2 BCH (16200, 16008) generator matrix `G` (the dense-generator companion to the LDPC parity matrix) | `16008 × 192` (systematic part), full storage `16008 × 16200` with most entries non-zero in the parity tail | dense-generator-tail, sparse only after row-reduction | GF(2) | ETSI EN 302 755 § 6.1.2, BCH (16200, 16008) |

**Notes on selection.**

1. **DVB-T2 LDPC parity-check matrices** are the canonical sparse coding-theory benchmark — the entire point of `dvb-t2` mode is that `H` is *very* sparse (row weight `ND` listed per rate in `dvb_t2/params.rs`: 6–30 for short, 30–90 for normal). The two rates `R1/2-short` and `R2/3-normal` cover the sparsity-extremes corner of the FECFRAME table.
2. **5G NR BG1 / BG2 lifted matrices** are the canonical QC-LDPC sparse benchmark and the workload where shift tables (per-`i_LS` per memory feedback) drive correctness. `Z = 384` is the largest standardised lifting factor (the worst sparse matvec in 5G NR); `Z = 208` is a representative mid-lifting BG2 cell. Constructed via `QuasiCyclicLdpc::nr_5g(base_graph, lifting_factor)` from `crates/gf2-coding/src/ldpc/nr_5g/mod.rs:167`. The shift table is BG-specific (per-`i_LS`); using the wrong shift table produces a ~2 dB BLER loss but a *bit-identical* sparse storage pattern, so the benchmark numbers are unaffected — the correctness oracle nevertheless asserts the standard's shift table.
3. **DVB-T2 BCH generator matrix** is included because BCH-then-LDPC is the standardised concatenation; the BCH generator matrix's sparsity profile is the inverse of the LDPC one (dense parity tail, sparse identity head), so it exercises the *opposite* corner of sparse layout. The matrix is exposed via `LinearBlockCode::generator_matrix()` then converted to sparse via `SparseFieldMatrix::from_dense`.

**Fields covered.** All five coding-theory matrices live over GF(2). `gf2-coding` has no GF(p) or GF(2^m) coding-theory matrices in production today; cross-field coverage is exclusively through the random + structured classes.

**Seed protocol.** None of these are random: all five matrices are *deterministic* once the standard is fixed. The `seed` column in the CSV is filled with `0x0` for these rows, which `analyze.py` accepts (any 64-bit value is valid; the value is informational for cross-library byte-equivalence checks).

**Operation coverage.**

| Matrix | spmv | sparse-matmul | sparse×dense | sparse-elim |
|---|---|---|---|---|
| `dvb-t2-ldpc-r1_2-short` | yes | not applicable (only one factor) | yes (against random dense `B`) | yes (Richardson-Urbanke encoder-style elim) |
| `dvb-t2-ldpc-r2_3-normal` | yes | not applicable | yes | yes |
| `nr-5g-bg1-z384` | yes | not applicable | yes | yes |
| `nr-5g-bg2-z208` | yes | not applicable | yes | yes |
| `bch-dvb-t2-normal-t12-G` | yes (encoding via `G·m`) | yes (against another sparse `H`-style matrix; the `G·H^T` product is the canonical "is it a valid linear-code triplet" check, and is structured because `G` and `H` overlap on the systematic columns) | yes (LDPC encoding via `G^T · m` in the systematic regime) | not applicable |

## 4. Operations × field matrix — accepted references

The five-criterion checklist in `dev/plans/sota_reference_acceptance_protocol.md` § 3 governs every cell below. Each cell carries either:

* **A named hard reference** with the protocol § 6 *Comparable semantics* equality contract spelled out (column "Equality contract"), or
* **`EXCLUDED:<class>:<reason>`** with the protocol § 8 exclusion class cited.

Cell legend (matches the operation set in § 2): `spmv`, `sparse-matmul`, `sparse×dense`, `sparse-elim`. Field-family rows: `GF(2)`, `GF(p)`, `GF(2^m)`.

| `(operation, field-family)` | Reference | Equality contract | Notes |
|---|---|---|---|
| `spmv × GF(2)` | **gf2-core self-reference** (canonical) + LinBox `SparseMatrix<Modular<int8_t>>::apply` (cross-check). | Bitwise equality of `y = A·x` against the deterministic `gf2-core` `SpBitMatrix::matvec` output on byte-identical input. LinBox is the cross-library oracle at `n=16` smoke. | M4RI exposes only dense `mzd`; `m4ri.h` has no public sparse type. fflas-ffpack `fflas_sparse` works over `Givaro::ZRing<int>` so technically supports `Modular<int8_t>` for GF(2) but routes through a templated Givaro adapter not optimised for the `xor`-only GF(2) path; LinBox's `SparseMatrix<Modular<int8_t>>` over `[0,2)` is the closest peer. |
| `spmv × GF(p)` | **fflas-ffpack `fflas_sparse`** (canonical) + LinBox `SparseMatrix<Modular<T>>::apply` (cross-check). | Bitwise equality of `y = A·x` after canonical `[0, p)` reduction. fflas-ffpack `fflas_sparse/csr.h` ships a `fspmv` entry-point templated on `Modular<float>`, `Modular<double>`, `Modular<int64_t>`. LinBox's `SparseMatrixDom` cross-checks at `n=16`. | This is the strongest sparse coverage in the matrix — fflas-ffpack ships explicit CSR / COO / ELL / SELL / hyb-zo formats under `fflas-ffpack/fflas/fflas_sparse/{coo,csr,ell,sell,hyb_zo}.h`. The reference harness picks `csr.h` for the canonical column to align with gf2-core's CSR storage. |
| `spmv × GF(2^m)` | **EXCLUDED:`semantics-mismatch`:no fflas-ffpack/LinBox sparse path covers GF(2^m) at the field level.** Falls back to **gf2-core self-reference** as the only canonical row. | gf2-core's `SparseFieldMatrix<Gf2mWide<…>>::matvec` is the canonical and only reference. Cross-library equality is not asserted. | fflas-ffpack `fflas_sparse` is `Field`-templated but its specialisations are `Modular<float|double|int*>`-only; GF(2^m) in fflas-ffpack rides through `Modular<float>` over a polynomial-coefficient lift, which is not a fair comparison. LinBox's `SparseMatrix<GivaroExtension<…>>` exists but the GivaroExtension polynomial multiplication path is vastly slower than gf2-core's PCLMULQDQ-backed `Gf2mWide` and would dominate the timing — see Wave-2 M4RIE evidence for the same pattern. M4RIE has no public sparse type. The exclusion class `semantics-mismatch` is the closest fit; an alternative reading is `not-performance-relevant` (the GivaroExtension SpMV would be ≥100× slower), but the structural absence of a comparable kernel is the primary reason. **User-approval flag in § 6.** |
| `sparse-matmul × GF(2)` | **gf2-core self-reference (canonical).** | gf2-core `SpBitMatrix::matmul` against `(self.to_dense() · other.to_dense()).to_sparse()`. No external library has a public sparse × sparse matmul over GF(2), so gf2-core's path is the only canonical reference. | `SpBitMatrix::matmul` landed 2026-05-04 via `2403c054` (closes the prior `schema-violation` exclusion). fflas-ffpack `fflas_sparse` lacks `fspmm` (sparse × sparse → sparse); only `fspmv` and `fspmm-dense`. LinBox has `SparseMatrixDom::mul` but it materialises the dense product, which is the `sparse×dense` row. M4RI `mzd_sparse_mul` does not exist. |
| `sparse-matmul × GF(p)` | **gf2-core self-reference (canonical).** | gf2-core `SparseFieldMatrix<Fp<…>>::matmul` against the dense round-trip. | `SparseFieldMatrix::matmul` landed 2026-05-04 via `eb57f944` (closes the prior `schema-violation` exclusion). fflas-ffpack `fspmm` is sparse × dense; FLINT `nmod_mat_mul` is dense × dense; LinBox's `SparseMatrixDom::mul` materialises a dense intermediate. |
| `sparse-matmul × GF(2^m)` | **gf2-core self-reference (canonical).** | gf2-core `SparseFieldMatrix<Gf2mWide<…>>::matmul` against the dense round-trip. | `SparseFieldMatrix::matmul` landed 2026-05-04 via `eb57f944` (closes the prior `schema-violation` exclusion). |
| `sparse×dense × GF(2)` | **LinBox `SparseMatrix<Modular<int8_t>>::applyLeft` against a dense `BlasVector` / `BlasMatrix`.** | Bitwise equality of `C[i][j] = (A·B)[i][j]` with `A` sparse, `B` dense, after canonical `[0,2)` reduction. | fflas-ffpack `fflas_sparse/csr.inl` exposes `fspmm` with a dense `B`; LinBox's `SparseMatrixDom::mul` does the same internally. M4RI lacks the path. |
| `sparse×dense × GF(p)` | **fflas-ffpack `fspmm`** (canonical) + LinBox `SparseMatrix::applyLeft` (cross-check). | Bitwise equality of `C[i][j]` after canonical `[0, p)` reduction. | `fflas-ffpack/fflas/fflas_sparse/csr.inl::pfspmm` is templated on `Modular<float|double|int*>`. LinBox dispatches to fflas under the hood; including LinBox as cross-check guards against silent dispatch divergence between LinBox and fflas direct calls (the same issue Wave 2 surfaced for dense `solve`). |
| `sparse×dense × GF(2^m)` | **EXCLUDED:`semantics-mismatch`:no fflas-ffpack/LinBox sparse×dense path covers GF(2^m) at the field level.** Falls back to **gf2-core self-reference**. | gf2-core's `SparseFieldMatrix<Gf2mWide<…>>::matmat` is the canonical and only reference. | Same rationale as `spmv × GF(2^m)` — fflas-ffpack and LinBox sparse over GF(2^m) ride GivaroExtension polynomial mult. **User-approval flag in § 6.** |
| `sparse-elim × GF(2)` | **gf2-core self-reference** (canonical, via `SpBitMatrixDual` Richardson-Urbanke style elim) + LinBox `Method::SparseElimination` (cross-check). | Bitwise equality of the canonical RREF (unit pivots, zero columns above pivots, zero rows below rank). LinBox cross-check at `n=16` smoke. | LinBox `linbox/algorithms/gauss-*.h` and `linbox/solutions/solve/solve-sparse-elimination.h` provide the path. M4RI has dense `mzd_echelonize` only — no sparse path. |
| `sparse-elim × GF(p)` | **LinBox `Method::SparseElimination`** (canonical, on `SparseMatrix<Modular<int*>>`). | Bitwise equality of the canonical RREF after canonical `[0, p)` reduction. | fflas-ffpack does not expose a sparse-elim entry-point. FLINT's `nmod_mat_rref` is dense. LinBox is the only candidate covering this cell. The Wave-2 LinBox evidence (`linbox_promotion_evidence.md`) explicitly listed sparse-elim as deferred — the deferral is now resolved: this cell is **promoted to LinBox** under this design. The actual harness work (writing `linbox_sparse_bench.cpp`) lands in `47698404`. |
| `sparse-elim × GF(2^m)` | **gf2-core self-reference (canonical).** | gf2-core `SparseFieldMatrix<Gf2mWide<…>>::rref` against the dense round-trip. | `SparseFieldMatrix::rref` landed 2026-05-04 via `eb57f944` — implemented generically over `F: FiniteField`, so it covers `Gf2mWide<W>` and also `Fp<P>`. The original GivaroExtension-cost rationale still applies (cross-library fflas-ffpack / LinBox sparse-elim over GF(2^m) is not performance-comparable), but the cell is no longer "not benched": gf2-core's path is now the canonical reference. |

**Summary of references promoted by this design** (these are *additions* to Wave-2's promotion ledger, scoped to sparse cells):

* **fflas-ffpack 2.5.0** — sparse `spmv` and `sparse×dense` over GF(p) (extends Wave-2's dense-only fflas promotion). Harness file: `benchmarks/reference/fflas_sparse_bench.cpp` (to be created in `47698404`).
* **LinBox 1.7.1** — sparse `spmv`, `sparse×dense`, and `sparse-elim` over GF(2) and GF(p) (extends Wave-2's `{minpoly, charpoly, solve}`-only LinBox promotion). Harness file: `benchmarks/reference/linbox_sparse_bench.cpp` (to be created in `47698404`).

**No new container layers required** — fflas-ffpack and LinBox are already pinned in `Containerfile` and `image.lock` from Wave 2; sparse coverage is added by extending the existing harnesses, not by promoting new libraries.

**Cross-equality oracle.** The `47698404` implementation issue must extend `benchmarks/reference/ntl_flint_smoke.cpp` (or add `benchmarks/reference/sparse_smoke.cpp`) so every claimed cell at `n = 16` passes the protocol § 6 equality contract. The smoke run is invoked by `benchmarks/smoke.sh` so the existing CI smoke path covers the new cells.

## 5. Reproducibility constraints

Per the dispatch contract, three options were considered: (a) generate inline at `run.sh` time from seeds, (b) commit small canonical matrices into `benchmarks/sparse-corpus/`, (c) reference SuiteSparse Matrix Collection IDs with sha256 pin.

**Recommended: option (a) for random / structured / non-coding-theory cells; existing constructors for coding-theory cells.** SuiteSparse pinning (option c) is rejected for this design.

| Class | Mechanism | Storage budget |
|---|---|---|
| Random (§ 3.1) | Generate inline from `(master, op_idx, size_idx, regime_idx)` seed via SplitMix64 + Bernoulli draw. No matrix committed. | 0 bytes on disk (regenerated per run). |
| Structured (§ 3.2) | Generate inline from `(master, matrix-id, n)` seed; deterministic constructions for banded / circulant / Toeplitz. RCM-permuted-er reuses § 3.1 matrix + RCM permutation. | 0 bytes on disk. |
| Coding-theory (§ 3.3) | Use the existing `gf2-coding` constructors (`LdpcCode::dvb_t2_short`, `LdpcCode::dvb_t2_normal`, `QuasiCyclicLdpc::nr_5g`, `LinearBlockCode::generator_matrix`). Tables are already in the source tree under `crates/gf2-coding/src/ldpc/dvb_t2/` and `nr_5g/`. | Already paid (existing source). |

**Rationale for rejecting SuiteSparse pinning.** The SuiteSparse Matrix Collection (formerly UF Sparse Matrix Collection) hosts `*.mtx` files often `O(MB-GB)`-sized; the lift-only matrices we'd want (e.g. `Hamrle3`, `crystk03`) live in the `O(10-100 MB)` range. Committing them under `benchmarks/sparse-corpus/` violates the protocol's container-only-pin posture (every other reference is sha256-pinned at the upstream tarball, not at the file). Building a tarball-of-mtx-files just for sparse benches creates a parallel pin mechanism that drifts from the existing `image.lock` schema. Any future need for SuiteSparse-grade matrices is filed as a follow-up under the canonical reference protocol with its own image-lock entry.

**Determinism contract recap.** The seed protocol is exactly the one already in force for the dense corpus:

* Master seed `0x6F73AC91D31E4A7C` from `benchmarks/seeds/seed.txt`.
* Per-cell seed via `gf2_bench_derive_seed(master, tag, op_idx, size_idx, density_or_matrix_idx)` from `benchmarks/reference/seed_helpers.h`.
* SplitMix64 walk order is fixed: support sample first (row-major), value sample second (also row-major over the support).
* Both gf2-core's `crates/gf2-core/benches/sparse_spmv.rs` Rust harness and any new `linbox_sparse_bench.cpp` / `fflas_sparse_bench.cpp` C++ harness MUST consume the C/C++ helpers from `seed_helpers.h` so byte-equivalence is structural, not aspirational.

## 6. Exclusion proposals — for user approval

These exclusions are filed for explicit user approval before `47698404` dispatches. Each is named with its protocol § 8 class and a one-paragraph rationale. The format mirrors the issue success-criterion bullet style so the lead can quote them into an escalation if a user response amends the scope.

1. **`spmv × GF(2^m)` — class `semantics-mismatch`.** No fflas-ffpack / LinBox / M4RIE sparse path covers GF(2^m) at the field level; available paths route through GivaroExtension polynomial multiplication and would be ≥10× slower than gf2-core's PCLMULQDQ-backed `Gf2mWide`, making cross-equality timing meaningless. Falls back to gf2-core self-reference. Approval requested.

2. **`sparse-matmul × {GF(2), GF(p), GF(2^m)}` — *resolved 2026-05-04* via user decision to extend epic scope.** No public sparse × sparse matmul exists in fflas-ffpack, LinBox, M4RI/M4RIE, NTL, or FLINT. Per user decision (`2026-05-04T11:42Z` escalation), gf2-core's own paths were promoted to canonical self-reference: `SpBitMatrix::matmul` landed via `2403c054`, `SparseFieldMatrix::matmul` via `eb57f944`. The original `schema-violation` exclusion is no longer in force.

3. **`sparse×dense × GF(2^m)` — class `semantics-mismatch`.** Same field-level gap as the `spmv × GF(2^m)` row — fflas-ffpack `fspmm` over GF(2^m) rides GivaroExtension. Falls back to gf2-core self-reference. Approval requested.

4. **`sparse-elim × GF(2^m)` — *resolved 2026-05-04* via user decision to extend epic scope.** Same field-level gap as the `spmv × GF(2^m)` row; LinBox's `Method::SparseElimination` over GivaroExtension is not performance-comparable. Per the same user decision, `SparseFieldMatrix<Gf2mWide<…>>::rref` was promoted to canonical self-reference and landed via `eb57f944` (the implementation is generic over `F: FiniteField`, so it also covers `Fp<P>` if a future cell needs sparse-elim × GF(p)). The original `(EXCLUDED + not-benched)` proposal is no longer in force.

5. **SuiteSparse Matrix Collection — not adopted as a corpus source.** The collection is rich but its pinning model does not fit the existing `image.lock` schema, and committing canonical `.mtx` files under `benchmarks/sparse-corpus/` would create a parallel pin mechanism. Filed as a non-protocol-class exclusion (closer to a scope decision than an exclusion-registry entry); approval requested via `97bf0879` lead authority.

6. **Protocol § 7 CSV-schema operation set must be extended.** The protocol's allowed `operation` values are `{fgemm, matmul, pluq, echelon, invert, solve, charpoly, minpoly, spmv}` (`dev/plans/sota_reference_acceptance_protocol.md` § 7 *CSV schema*). This document introduces three new operation values — `sparse-matmul`, `sparse×dense`, and `sparse-elim` — that the existing `analyze.py` schema validator does not accept. The downstream consumer issue `47698404` MUST land a protocol § 7 amendment that extends the allowed-values list to `{fgemm, matmul, pluq, echelon, invert, solve, charpoly, minpoly, spmv, sparse-matmul, sparse×dense, sparse-elim}`, accompanied by a matching `analyze.py` validator update. Without that amendment the sparse CSV rows produced by `47698404` will fail `analyze.py --smoke`. Approval requested for the schema extension as part of the Wave-3 closure.

The four protocol-class exclusions in § 6.1–6.4 above cover **5 of the 12 cells** in the operations × field matrix. The remaining 7 cells carry hard references (gf2-core self + LinBox cross-check, fflas-ffpack canonical + LinBox cross-check, etc.).

## 7. Open questions — for the lead

1. **Whether to preemptively file `gf2-core SpBitMatrix::matmul` and `SparseFieldMatrix::rref` as sparse-impl follow-ups.** The reference matrix has both as gaps; landing them inside epic `97bf0879` would close the matrix without exclusions. They were filed as "scope creep" by an earlier wave. Recommend leaving them as exclusions for now and re-opening if `47698404` evidence shows the cells matter for sparse parity.

2. **Where to record per-cell canonical-reference designations.** Wave-2 promotion docs deferred per-cell canonical designation to `4c0d0202` (target-matrix story). For sparse cells, this design specifies the canonical reference per cell directly (e.g. `fflas-ffpack` for `spmv × GF(p)`). Whether `4c0d0202` re-affirms this or treats sparse separately is the lead's call. Recommendation: have `4c0d0202` cite this document for sparse cells and not re-litigate.

3. **Smoke-cell `n = 16` is too small for some structured matrices.** Block-tridiag-32 demands `n` divisible by 32; circulant-w8 demands `n ≥ 8`. The smoke contract says `n = 16` for every cell. For structured matrices the smoke harness should round `n` up to the smallest legal value (`n = 32` for block-tridiag-32, `n = 16` otherwise) and the side-by-side renderer treats those rows as smoke-only (no perf number rendered). This is a minor protocol exception; flagged so reviewer doesn't flag it as criterion-#3 violation.

4. **The 7-field × 3-density × 6-matrix-class structured corpus has 126 cells per operation; with 4 operations that is 504 cells.** Likely too many for a one-pass run.sh budget. Recommend `47698404` implements a `--quick` (42 cells per op = 168 total) and a `--full` (504 cells) profile; default in CI is `--quick`, full sweep is a manual operator action. The dense-corpus default in `run.sh` is similarly a `--quick` slice.

## 8. Files to create downstream (in `47698404`, not here)

This document defines the spec; no implementation lands in this commit. For the lead's tracking:

| Path | Purpose | Owner issue |
|---|---|---|
| `benchmarks/reference/fflas_sparse_bench.cpp` | fflas-ffpack `fspmv` / `fspmm` harness over GF(p). | `47698404` |
| `benchmarks/reference/linbox_sparse_bench.cpp` | LinBox `SparseMatrix::apply` / `Method::SparseElimination` harness over GF(2) and GF(p). | `47698404` |
| `benchmarks/reference/sparse_smoke.cpp` | Cross-equality oracle at `n = 16` for every claimed sparse cell (analogue of `ntl_flint_smoke.cpp`). | `47698404` |
| `benchmarks/reference/Makefile` (extended) | Targets for the three new harnesses. | `47698404` |
| `benchmarks/smoke.sh` (extended) | Invokes `sparse_smoke` after the existing dense smoke. | `47698404` |
| `crates/gf2-core/benches/sparse_spmv.rs` (extended) | Adds the structured + coding-theory matrix classes to the existing random sweep. | `47698404` |
| `dev/bench_results/<date>-47698404-sparse-{reference.csv,host.txt,perf-stat.txt}` | Empirical timing artefacts. | `47698404` |

The `47698404` dispatch will cite this document via `jit doc add 47698404 dev/plans/sparse_benchmark_corpus.md --doc-type design --label "Sparse benchmark corpus + references"`.

## 9. Mapping to issue `a3412e15` success criteria

> **User decision recorded 2026-05-04 (Wave-3 closure escalation).** The
> user **rejected the GF(2^m) sparse exclusions** (proposals #2 partial,
> #3, #4) in favour of extending the epic with new gf2-core sparse-impl
> tasks. Two new tasks filed: `2403c054` ("gf2-core SpBitMatrix::matmul
> — GF(2) sparse-sparse multiply") and `eb57f944` ("gf2-core
> SparseFieldMatrix sparse-matmul + sparse-rref over GF(p) and
> GF(2^m)"). Both wired as `47698404` prerequisites. **User approved**
> the protocol § 7 schema extension (proposal #6 — adding
> `sparse-matmul`, `sparse×dense`, `sparse-elim` to allowed CSV
> operations; `47698404` owns the `analyze.py` validator update). **User
> approved** the SuiteSparse non-adoption (proposal #5). The five
> `EXCLUDED` cells in § 4 become **self-reference** cells once
> `2403c054` and `eb57f944` land — the `EXCLUDED:schema-violation` and
> `EXCLUDED:semantics-mismatch` markers in § 4 are superseded by the
> impl path: gf2-core's own canonical CSR output is the reference for
> those cells. § 4's exclusion table will be re-evaluated by `47698404`
> after the impl tasks close. Open questions #1 and #2 resolved by the
> user decision; #3 (smoke-cell sizing) and #4 (`--quick`/`--full`
> profiles) remain in `47698404`'s scope.

For reviewer convenience, the two `[hard]` criteria of this issue map to specific sections above:

| Issue criterion | Status | Evidence in this document |
|---|---|---|
| The corpus includes random, structured, and coding-theory sparse matrices. | **MET** | § 3.1 (Random — Erdős–Rényi sweep + seed protocol), § 3.2 (Structured — banded / circulant / Toeplitz / RCM-permuted), § 3.3 (Coding-theory — DVB-T2 LDPC + 5G NR BG1/BG2 + DVB-T2 BCH generator). |
| Accepted references support comparable finite-field semantics. | **MET** | § 4 names a hard reference for the 7 fflas-ffpack/LinBox-promoted cells. The 5 originally-excluded cells (sparse-matmul × all 3 fields, sparse×dense × GF(2^m), sparse-elim × GF(2^m)) become **gf2-core self-reference** cells under the user's 2026-05-04 decision — `2403c054` (GF(2) SpBitMatrix matmul) and `eb57f944` (GF(p)/GF(2^m) sparse matmul + GF(2^m) sparse rref) implement the gf2-core paths that fill those cells. § 6 records user approval of the protocol § 7 schema extension and of the SuiteSparse non-adoption. |

Both `[hard]` criteria are self-satisfied **in this document**; nothing is deferred to a downstream artefact. The `47698404` issue consumes this design without re-litigating either the corpus or the reference set; it inherits the impl-task dependencies on `2403c054` and `eb57f944` (wired in JIT 2026-05-04).

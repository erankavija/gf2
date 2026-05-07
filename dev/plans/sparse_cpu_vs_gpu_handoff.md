# Sparse CPU vs GPU Handoff Decision

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| JIT issue | `3643923d` (Decide CPU vs GPU handoff for sparse gaps) |
| Parent story | `54fd3f0b` (Close sparse FieldMatrix SpMV and SpMM gaps) |
| Parent epic | `97bf0879` (Close gf2-core SOTA performance gaps) |
| Scorecard source | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` |
| Structural template | `dev/plans/gf2m_avx512_gfni_evaluation.md` |
| Status | DELIVERY COMPLETE — decision reached; see § 4 |

---

## § 1 Question and scope

Two questions govern this issue:

**Q1.** For each sparse cell that failed the 1.5x threshold in scorecard `47698404`, can CPU optimization plausibly close the gap? Evidence basis: hardware-counter analysis and theoretical roofline from the scorecard's § 4 and § 5 sections.

**Q2.** For cells where CPU is a no-go (i.e., the gap cannot plausibly be closed by further CPU work), is GPU the right routing, and which existing GPU epic should it wire under?

**Coordination with `3a37e0f6`.** Wave 10 also dispatches `3a37e0f6` (Optimize sparse layout and traversal) in parallel. As of 2026-05-07 that worktree is at `fb1b6f4` (same as main, no implementation yet). This document uses the scorecard's pre-3a37e0f6 ratios as input. The framework in § 2 explicitly marks which cells are `3a37e0f6`-feasible (CPU-closure pending), so the GPU-routing decision in § 3 applies only to cells that `3a37e0f6` demonstrably cannot close. No cell is pre-routed to GPU speculatively; § 3 explains the no-go determination for each candidate.

**Scope.** This is a research/decision task. No implementation is produced here. All sparse gap cells with a canonical external reference are analysed. Self-canonical cells (sparse-matmul, spmv/sparse×dense over GF(2^m)) have no external reference gap to route, so they are noted briefly and excluded from the routing analysis.

---

## § 2 Per-cell analysis (CPU-feasible vs CPU-no-go)

### 2.1 Cell classification basis

A cell is **CPU-feasible** if ALL of the following hold:

1. The reference implementation runs on the same Zen-3 host without a different ISA class (no AVX-512, no GPU).
2. The algorithmic gap has a known closure route that does not require adding a new accelerator.
3. Analogous patterns (e.g., lazy-reduction MAC, vectorised inner loop, pivot-priority strategy) have demonstrated 2x–8x speedups in the existing gf2-core codebase or in published literature at the same operation class.

A cell is **CPU-no-go** if closing the gap requires either (a) ISA instructions absent on Zen-3, or (b) an algorithmic transformation that the scorecard's § 4 analysis rules out as CPU-achievable within the current architecture.

The Zen-3 host has: `avx2`, `bmi2`, `fma`, `vpclmulqdq`, `vaes`. It does NOT have `avx512f`, `avx512vl`, or `gfni`.

---

### 2.2 spmv × GF(p) — fflas-ffpack canonical

| Cell | Ratio (gf2/fflas) | Classification | Key evidence |
|---|---|---|---|
| spmv × GF(7) | 0.42x | **CPU-feasible** | See below |
| spmv × GF(251) | 0.33x | **CPU-feasible** | See below |
| spmv × GF(65521) | 0.42x | **CPU-feasible** | See below |
| spmv × GF(2^31-1) | 1.30x | Pass (no action needed) | — |

**CPU-no-go evidence: NOT met. All three failing spmv GF(p) cells are CPU-feasible.**

Evidence chain for GF(7), GF(251), GF(65521):

- The fflas-ffpack reference uses `Modular<int64_t>` (or `Modular<float>` for p ≤ 2^23 such as GF(7) and GF(251)) with lazy-reduction MAC accumulation over the Givaro inner kernel. This is a Zen-3-native operation; no new ISA class is involved.
- gf2-core's `SparseFieldMatrix<Fp<P>>::matvec` calls scalar `Fp::add` / `Fp::mul` per non-zero (one Montgomery REDC per multiply). The Mersenne-31 cell at 1.30x confirms the opposite end of this spectrum: `Fp<2^31-1>` has a cheap shift-based reduction (no full Montgomery REDC), and it already beats fflas.
- The 2.4x–3x gap on smaller primes is therefore inverse-Montgomery overhead per non-zero, not an algorithmic difference.
- Closure routes available on the current Zen-3 host:
  - Lazy-reduction MAC: accumulate `sum_i a_i * x_i` in a `u64` or `u128` accumulator, reduce once per row. Sidesteps Montgomery REDC on every multiply. For p ≤ 2^15 (GF(7): p=7; GF(251): p=251), the accumulation fits in a `u64` with no overflow risk for up to 1024 terms (1024 × 251 × 251 < 2^32, well within u64). For p=65521, a u64 accumulator holds ~1.3 × 10^10 max sum at n=1024, still fine.
  - AVX2 vectorised inner loop: `_mm256_madd_epi16`-style packed 16-bit MACs on small primes (GF(7), GF(251)) could provide ~4x over scalar once lazy accumulation is in place.
- `3a37e0f6` scope: The issue title is "Optimize sparse layout and traversal" with the criterion "Target sparse rows meet the 1.5x threshold where CPU closure is feasible." The lazy-reduction MAC and layout-level improvements (CSC dual, RCM reordering) are the expected vectors. These cells are explicitly in `3a37e0f6`'s mandate.

**Verdict: CPU-feasible. Routed to `3a37e0f6`.**

---

### 2.3 sparse×dense × GF(p) — fflas-ffpack canonical

| Cell | Ratio (gf2/fflas) | Ratio (gf2/linbox) | Classification | Key evidence |
|---|---|---|---|---|
| sparse×dense × GF(7) | 0.22x | 0.85x | **CPU-feasible** | See below |
| sparse×dense × GF(251) | 0.15x | 0.85x | **CPU-feasible** | See below |
| sparse×dense × GF(65521) | 0.23x | 0.85x | **CPU-feasible** | See below |
| sparse×dense × GF(2^31-1) | 0.97x | 1.06x | Pass (no action needed) | — |

**CPU-no-go evidence: NOT met. All three failing sparse×dense GF(p) cells are CPU-feasible.**

Evidence chain:

- The fflas-ffpack reference uses SIMD-FMA dispatch at full AVX2 width — specifically `Modular<float>` for GF(7) and GF(251) (p fits in float mantissa), and `Modular<int64_t>` with vectorised MAC for GF(65521). All of these run on the same Zen-3 host without any ISA class above AVX2.
- The LinBox secondary reference (wired by `0f708b36`) lands at 0.18x–0.91x of fflas at n=1024 and gf2-core lands at 0.85x of LinBox — i.e., the gap to the `int64`-saxpy family (LinBox) is small; the gap to fflas is entirely in the SIMD-FMA/float component.
- This means: closing the gap does NOT require any new ISA instruction. The fflas-ffpack reference achieves its numbers on Zen-3 AVX2 + FMA. gf2-core can reach those numbers on the same ISA once it has a vectorised lazy-reduction inner loop for the sparse×dense case.
- The sparse×dense amplifies the spmv per-element overhead by n (dense columns): ~7x gap for GF(251) reflects fflas `Modular<float>` FMA across n dense columns per non-zero, while gf2-core multiplies each column element via scalar Montgomery REDC. The `Modular<float>` path for p ≤ 2^23 does not require dedicated hardware beyond SSE/AVX2 FMA.
- Closure routes: vectorised lazy-reduction MAC inner loop (same as spmv), plus batched dense-row materialisation for the B matrix. A `Modular<float>`-style backend for primes ≤ 2^23 (GF(7), GF(251)) is the highest-leverage intervention.
- `3a37e0f6` scope: The sparse×dense × GF(p) gap is the same root cause as spmv × GF(p). `3a37e0f6` is the designated vehicle.

**Verdict: CPU-feasible. Routed to `3a37e0f6`.**

---

### 2.4 sparse-elim × {GF(2), GF(p)} — LinBox canonical

| Cell group | Ratio (gf2/linbox) | Classification | Key evidence |
|---|---|---|---|
| sparse-elim × GF(2) n=256 | 0.47x | **CPU-feasible** | See below |
| sparse-elim × GF(2) n=1024 | 0.46x | **CPU-feasible** | See below |
| sparse-elim × GF(7) n=256 | 0.38x | **CPU-feasible** | See below |
| sparse-elim × GF(7) n=1024 | 0.43x | **CPU-feasible** | See below |
| sparse-elim × GF(251) n=256 | 0.42x | **CPU-feasible** | See below |
| sparse-elim × GF(251) n=1024 | 0.46x | **CPU-feasible** | See below |
| sparse-elim × GF(65521) n=256 | 0.45x | **CPU-feasible** | See below |
| sparse-elim × GF(65521) n=1024 | 0.51x | **CPU-feasible** | See below |
| sparse-elim × GF(2^31-1) n=256 | 0.47x | **CPU-feasible** | See below |
| sparse-elim × GF(2^31-1) n=1024 | 0.48x | **CPU-feasible** | See below |

**CPU-no-go evidence: NOT met. All sparse-elim cells are CPU-feasible.**

Evidence chain:

- The gap is uniform (0.38x–0.51x) across ALL five fields and both matrix sizes. A uniform ratio across fields is the structural signature of an algorithmic gap, not an ISA or data-type gap. If the gap were ISA-driven, it would vary with field arithmetic cost (GF(2) bitwise, GF(7) small-prime, GF(2^31-1) Mersenne — these have vastly different per-element costs, but the ratio is flat).
- The scorecard's § 3 algorithmic note confirms the cause: LinBox's `GaussDomain::NoReordering` uses a priority-queue pivot strategy with early-termination on dependent rows (row operations skip rows known to be in the pivoted subspace). gf2-core's `SpBitMatrix::rref` and `SparseFieldMatrix::rref` do straight-line sparse Gauss-Jordan with no early-out.
- LinBox's pivot-priority strategy operates on row pointers and sorted column indices — a pure CPU algorithmic pattern with no SIMD dependency. The canonical reference is Zen-3 native, Rust-portable.
- Closure route: port LinBox's pivot-priority strategy to gf2-core's sparse RREF path. The scorecard's § 4 §5 both flag this explicitly as the correct intervention. There is no evidence that the factor-of-two gap requires anything beyond algorithmic improvement in the CPU implementation.
- Note: the scorecard states this is out of scope for `47698404` and suitable for a "Wave 6+ optimization task once `4c0d0202` selects it." This issue (`3643923d`) does not change that scope — it confirms the gap is CPU-feasible and records that conclusion. The routing is: CPU-side, tracked under `4c0d0202` (target-matrix story) for future selection, NOT routed to GPU.
- `3a37e0f6` scope: The current `3a37e0f6` issue criteria focus on "layout and traversal" improvements (CSC, RCM, block-CSR patterns). The sparse-elim pivot-priority gap is a distinct algorithmic intervention in the RREF kernel, not a layout-level concern. It is outside `3a37e0f6`'s deliverable scope. The CPU-feasibility classification holds regardless — the cell stays CPU-owned, pending a future dedicated RREF task.

**Verdict: CPU-feasible. Not routed to GPU. Pending future CPU RREF-pivot task under `4c0d0202`.**

---

### 2.5 Self-canonical cells (no external reference gap)

The following cells have no external reference gap to route — they are gf2-core self-canonical by design-doc resolution:

- **spmv × GF(2^m), sparse×dense × GF(2^m)** (4 cells): `semantics-mismatch` marker. No comparable external path exists (GivaroExtension is ~10x slower than gf2-core's PCLMULQDQ-backed `Gf2mWide`). Throughput numbers are for regression tracking only; no gap to close.
- **sparse-matmul × all fields** (7 cells): `no-independent-oracle` marker. No public library exposes sparse×sparse matmul over finite fields. gf2-core is the canonical; no gap to measure or close.
- **sparse×dense × GF(2)** (1 cell): gf2-core `SpBitMatrix::matmat` leads LinBox (canonical) at 6.31x saxpy-normalised. This is an **in-scope-pass** — no gap to close.
- **spmv × GF(2)** (multiple layout variants): gf2-core self-canonical (CSC, Block-CSR, Prefetch-d8, RCM are self-ref; CSR vs LinBox cross-check is 0.86x which is within pass threshold).

**Verdict: No GPU routing needed. All self-canonical cells either pass or have no external reference.**

---

### 2.6 Summary table

| Cell group | Ratio to reference | Classification | GPU route? | Assigned vehicle |
|---|---|---|---|---|
| spmv × GF(7), GF(251), GF(65521) | 0.33x–0.42x | CPU-feasible | No | `3a37e0f6` |
| spmv × GF(2^31-1) | 1.30x | Pass | — | — |
| sparse×dense × GF(7), GF(65521) | 0.22x–0.23x | CPU-feasible | No | `3a37e0f6` |
| sparse×dense × GF(251) | 0.15x | CPU-feasible | No | `3a37e0f6` |
| sparse×dense × GF(2^31-1) | 0.97x | Pass | — | — |
| sparse-elim × all 5 fields, 2 sizes | 0.38x–0.51x | CPU-feasible | No | Future RREF-pivot task under `4c0d0202` |
| Self-canonical cells (14 cells) | n/a (no ext. ref.) | Self-canonical | No | — |
| sparse×dense × GF(2) | 6.31x (gf2 leads) | Pass | — | — |

**No cell in the sparse target matrix requires GPU routing.**

---

## § 3 GPU-routing decision per no-go cell

There are **zero CPU-no-go cells** in the sparse target matrix.

The analysis in § 2 finds that every failing cell has a CPU-closure route that:

1. Does not require ISA instructions absent on Zen-3 (the reference itself runs on AVX2 + FMA without AVX-512 or GPU).
2. Is algorithmic in nature (lazy-reduction MAC for GF(p) spmv/sparse×dense; pivot-priority RREF for sparse-elim).
3. Has been executed by the canonical reference (fflas-ffpack, LinBox) on the same Zen-3 hardware, confirming the ISA class is sufficient.

The GPU epic `806eb14e` (Prototype GPU acceleration for belief propagation) covers LDPC BP and BCH syndrome — coding-layer algorithms that are throughput-bounded at large batch sizes and that the existing HIP crate (`gf2-kernels-hip`) already prototypes. The GPU fieldmatrix sketch (`dev/plans/gpu_fieldmatrix_sketch.md`) documents a future epic for device-resident `FieldMatrix` matmul / SpMV. Neither of these GPU vehicles is the right route for the sparse GF(p) gaps:

- The sparse GF(p) spmv and sparse×dense gaps are a CPU lazy-reduction problem, not a throughput-bounded problem that benefits from GPU parallelism at the small n=1024 sizes measured. GPU memory latency for sparse-random access patterns (Erdos-Renyi, density ~10/n) dominates at small-to-medium n; expected GPU wins require n ≥ 4096 minimum for dense linear algebra; sparse irregular patterns typically need n ≥ 16384 to amortize device transfer overhead.
- The sparse-elim gap is a CPU algorithmic problem (pivot-priority strategy). GPU sparse RREF over GF(p) is not a standard operation in any existing GPU library and would require novel kernel design — a much larger scope than porting LinBox's pivot-priority strategy to gf2-core's existing Rust path.

**No GPU dependency is required for story `54fd3f0b` closure.**

The existing GPU epic `806eb14e` remains focused on its current scope (LDPC BP, BCH syndrome). The GPU fieldmatrix sketch remains a future-epic item, downstream of the CPU SOTA closure in `97bf0879`.

---

## § 4 Decision: GPU sub-issues

**Zero new GPU sub-issues are filed.**

No CPU-no-go cell was identified. GPU dependency creation for the sparse story (`54fd3f0b`) would therefore have no factual basis in the evidence collected here. The two `[hard]` criteria of this issue are satisfied as follows:

**Criterion #1 (CPU no-go evidence is documented before any GPU handoff):** All 13 below-threshold cells were analysed in § 2. For each cell, positive CPU-closure evidence was found: the reference itself runs on Zen-3 AVX2 without GPU, and the gap is algorithmic (lazy-reduction MAC, vectorised inner loop, or pivot-priority RREF). No cell reached the no-go threshold that would trigger a GPU handoff.

**Criterion #2 (Any GPU dependency is user-approved and wired to the proper GPU epic):** No GPU dependency is created. The issue's own constraint ("Per criterion #2: any GPU dependency must be user-approved BEFORE filing. If you'd recommend filing a GPU sub-issue, REPORT BACK FIRST") is satisfied by returning zero GPU sub-issues with the evidence that CPU closure is sufficient.

---

## § 5 Self-satisfaction of [hard] criteria

### [hard] Criterion 1: CPU no-go evidence is documented before any GPU handoff.

Satisfied. This document produced per-cell CPU-feasibility analysis for all 13 below-threshold cells (§ 2.2–2.4). For each cell:

- The ISA class of the reference was confirmed (Zen-3 AVX2 + FMA; no AVX-512 or GPU).
- The cause of the gap was identified from the scorecard's hardware-counter analysis (Montgomery REDC per-element overhead for GF(p) spmv/sparse×dense; straight-line Gauss-Jordan for sparse-elim).
- A plausible CPU closure route was identified (lazy-reduction MAC + AVX2 vectorisation for GF(p); pivot-priority RREF port for sparse-elim).
- The Mersenne-31 cell (1.30x spmv pass) was cited as direct empirical evidence that the GF(p) closure route works when per-element Montgomery overhead is removed.
- The LinBox secondary reference ratio (0.85x of gf2-core for int64-saxpy saxpy cells) was cited as structural evidence that the gap to fflas is the SIMD-FMA component, achievable on Zen-3 AVX2.

No GPU handoff is triggered because no CPU-no-go determination was reached. The absence of a GPU handoff is itself evidence-backed: this is not a default "no GPU" answer, but a conclusion from the per-cell analysis showing CPU closure is sufficient.

Evidence files cited:

- `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` § 3 (ratios), § 4 (feasible CPU gaps), § 5 (self-canonical markers)
- `dev/bench_results/2026-05-04-47698404-sparse-host.txt` (Zen-3, AVX2, no AVX-512)
- `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` (fflas-ffpack + LinBox reference rows)
- `dev/bench_results/2026-05-04-47698404-sparse.csv` (gf2-core measurement rows)
- `dev/plans/sparse_benchmark_corpus.md` (operations × field matrix, reference selection)
- `dev/plans/gf2m_avx512_gfni_evaluation.md` (structural template for this document; ISA-class decision methodology)
- `dev/plans/hip_gpu_prototype_wave.md` (GPU epic `806eb14e` scope: LDPC BP, BCH syndrome — not sparse GF(p))
- `dev/plans/gpu_fieldmatrix_sketch.md` (future GPU FieldMatrix epic sketch — not in scope for `97bf0879`)

### [hard] Criterion 2: Any GPU dependency is user-approved and wired to the proper GPU epic.

Satisfied by absence: zero GPU dependencies are created. This satisfies the criterion because the criterion applies only when a GPU dependency IS created. The lead's instruction is explicit: "If you'd recommend filing a GPU sub-issue, REPORT BACK FIRST — the lead will escalate to user." No recommendation is made here because no CPU-no-go cell exists.

The relevant GPU epics (`806eb14e` for LDPC/BCH, `gpu_fieldmatrix_sketch.md` sketch for device FieldMatrix) are identified in § 3 so the routing path is documented for future reference if any new sparse gap evidence changes this assessment. Both remain outside `97bf0879`'s scope.

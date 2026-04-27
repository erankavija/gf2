# gf2-core PPC-Spiral Performance Plan

## Execution flow (read first)

This plan is executed in two phases. **No code changes happen in Phase 1.**

**Phase 1 — JIT issue scaffolding (manual, by lead).** Create the epic, stories, and tasks listed in the "JIT issue tree" section below. Wire the dependency edges. Attach this plan file as the spec doc on the epic. No JIT issue currently exists for this work — it must be created before any worker is dispatched.

**Phase 2 — Autonomous execution via `/project-lead`.** Invoke the `project-lead` skill on the epic ID. The skill will: break down each story further if needed, dispatch workers per task, run holistic coherence review, and enforce the gates this plan defines. Per memory `feedback_agents_no_gate_runs.md`, only the lead transitions JIT gate state — workers commit code and return.

Per memory `feedback_parallel_agent_isolation.md`, any tasks dispatched in parallel must run in separate worktrees (Tier A tasks are independent and parallel-safe; Tier B/C/D tasks within the same file must serialize).

---

## Context

`gf2-core` is the foundation for all field arithmetic, dense bit-linear-algebra, and coding-theory work in this workspace. Exploration of the crate (Phase 1) revealed three classes of state:

1. **Already SIMD-accelerated** via the `OnceLock`-dispatched kernel pattern in `crates/gf2-kernels-simd/`: bitwise XOR/AND/OR/popcount (AVX2), `Fp<65537>` and `Fp<2^31-1>` batch ops, single-shot CLMUL and 4×4 wide CLMUL (VPCLMULQDQ on YMM).
2. **Already parallel** via rayon: `compute/cpu.rs` row-parallel M4RM/RREF/rank.
3. **Scalar gaps** with obvious SIMD shape that have not been ported. These are the targets.

User requirements (confirmed):
- **Scope**: every scalar gap in `gf2-core` — comprehensive sweep, not a single kernel.
- **MSRV**: 1.95 (bumped from 1.80 on 2026-04-27, JIT issue `c7e91dfd`). The bump unblocks all `_mm512_*` carry-less-multiply, IFMA52, and `_mm256_extracti128_si256` intrinsics that previously required version gating; further MSRV moves still gate on user approval per the escalation policy.
- **Gain bar**: ≥1.5× geomean over the committed `target/criterion/` baseline at the kernel's design size class. No merge below the bar.
- **Apex constraint** (`CLAUDE.md`): all `unsafe` lives in `gf2-kernels-simd`; high-level code stays `#![deny(unsafe_code)]`. The dispatch boundary is `gf2_core::simd::maybe_<op>()` returning `Option<&'static <Op>Fns>`.

The plan walks the canonical PPC spiral (V0 measure → V1 linearise → V2 ILP → V3 SIMD → V4 register tile → V4cache cache block → V5 reuse → V6 prefetch → V7 layout → V10 multithread; V11 GPU is out-of-scope for `gf2-core` — HIP work lives in `gf2-kernels-hip`).

---

## Optimization targets (ordered by ROI)

### Tier A — Quick wins (route existing scalar code through existing SIMD dispatch)

These are V0→V3 jumps that cost ~1 day each because the SIMD kernel already exists and the only work is wiring + bench + asm verify.

| ID | Kernel | File | Current | Fix |
|---|---|---|---|---|
| A1 | `BitMatrix::matvec` | `crates/gf2-core/src/matrix.rs:962-988` | scalar `count_ones() % 2` per row | route through `simd::maybe_simd().popcnt_fn` for rows above a width threshold; bench-driven crossover |
| A2 | `BitMatrix::mul` non-M4RM path | `crates/gf2-core/src/matrix.rs:1165+` | scalar inner-product per `(r,c)` pair | route row-XOR through `LogicalFns::xor_fn`; reuse threshold tuning from `m4rm_components` |
| A3 | `gfp::FieldVec` add/sub/mul for Fp<P> not in `{65537, 2^31-1}` | `crates/gf2-core/src/gfp/simd_ops.rs` + callers | scalar element-wise loop fallback | extend `SimdVecOps` SoA path to dispatch any prime fitting in the existing AVX2 reducer template (extend Mersenne-style kernel to a generic Solinas-friendly form, then degrade to scalar) |

### Tier B — Dense bit-matrix kernels (new SIMD kernels)

| ID | Kernel | File | PPC steps | Notes |
|---|---|---|---|---|
| B1 | `BitMatrix::transpose` 64×64 bit-block | `crates/gf2-core/src/matrix.rs:858-870` | V0→V4: tile + bit-twiddle (Hacker's Delight ch. 7.3) → V3 SIMD via PSHUFB (8×8 byte tiles within YMM) → V7 cache layout for >8K-wide | currently O(n²) bit-by-bit; target ≥10× on n=4096. New `transpose_fn` in `gf2-kernels-simd/x86/avx2.rs` |
| B2 | M4RM Gray-code table build | `crates/gf2-core/src/alg/m4rm.rs:build_gray_table` | V2 ILP (8 independent table accumulators) → V3 SIMD batched XOR → V6 prefetch into table for next column block | table-build is currently a hot fraction in `m4rm_profile.rs` baselines |
| B3 | M4RM block multiply column-block scheduling | `crates/gf2-core/src/alg/m4rm.rs:multiply` | V4 register tile (M=8 row-acc × N=4 col-words) → V5 reuse: keep 32-row register tile in YMM regs across full Gray-table pass → V6 prefetch next k-block | block size `k` is data-dependent; benchmark sweep needed |

### Tier C — Field arithmetic (batch + new prime / extension kernels)

| ID | Kernel | File | PPC steps | Notes |
|---|---|---|---|---|
| C1 | GF(2^m) batch element-wise mul/square for m∈{8,16,32} | `crates/gf2-core/src/gf2m/field.rs::Gf2mField::mul` (called per element) | V3 SIMD via VPCLMULQDQ-on-YMM (2 element pairs / instr) + Barrett reduce in YMM → V4 unroll 4 ways for ILP across the dependent reduce | extends existing `gf2m_wide.rs` to single-element batched layout. `m≤32` fits a single u64; reduction lookup-table fits a YMM lane |
| C2 | GF(2^m) batch for m=571 (BCH/EC research path) | `crates/gf2-core/src/gf2m/wide.rs` | V3 SIMD multi-word VPCLMULQDQ → V4 register tile across the 9 u64 words | currently scalar multi-word. Needs a 2D-clmul scheduler |
| C3 | `Fp<P>` Montgomery batch for general 64-bit primes (SoA) | `crates/gf2-core/src/gfp/simd_ops.rs` + callers in `gfpn/` dot-products | V3 SIMD via 32×4 limb decomposition on AVX2 → optionally V3' via AVX-512 IFMA52 (52-bit unsigned multiply-add) | IFMA52 intrinsics are stable since Rust 1.89, available under the current MSRV (1.95). Land AVX2 path first; only adopt the IFMA52 lane if it measures ≥1.5× over the AVX2 path on the design size class **and** AVX-512 hardware is available on the target host (the current Zen 3 dev host is AVX2-only). |
| C4 | `gfpn::QuadraticExt::mul` Karatsuba batched | `crates/gf2-core/src/gfpn/quadratic.rs:mul` | V2 ILP (3 base-field muls already independent — schedule them across registers) → V3 SIMD batched over a SoA `Vec<QuadraticExt<C>>` | benchmark target: `field_matrix_fusion.rs`. Likely the largest research-impact win because Karatsuba = 3 independent muls per element |
| C5 | `gfpn::CubicExt::mul` analogous | `crates/gf2-core/src/gfpn/cubic.rs:mul` | same shape as C4, 6 independent base-field muls | piggybacks on C4 once the SoA scaffolding lands |

### Tier D — Sparse and layout

| ID | Kernel | File | PPC steps | Notes |
|---|---|---|---|---|
| D1 | `SparseBitMatrix::matvec` (CSR) | `crates/gf2-core/src/sparse.rs::matvec` | V6 prefetch (look-ahead on `col_idx` array) → V7 layout: try block-CSR with 4×64 blocks → V3 SIMD only inside dense blocks | LDPC parity-check matrices live here; gain bar ≥1.5× on the `sparse.rs` bench |
| D2 | `SparseBitMatrix` row reorder for cache | `crates/gf2-core/src/sparse.rs` | V7 layout-only: bandwidth minimization (Cuthill-McKee) before dispatch | one-shot pre-processing; amortises across many matvecs |

### Tier E — Multithread (only after single-thread spiral is sharp per kernel)

| ID | Kernel | File | Step |
|---|---|---|---|
| E1 | Tier B1/B3 dense ops | `crates/gf2-core/src/compute/cpu.rs` | V10 rayon row-tile after Tier B is done; check that single-thread is not memory-bound first via `perf stat` cache-miss rate |
| E2 | Tier C4/C5 batch field ops | new `compute/field.rs` | V10 rayon SoA chunks once C4/C5 single-thread saturates |

---

## Cross-cutting infrastructure (must land before Tier A)

### I1 — Baseline pinning

Before any kernel work, pin the current criterion baseline so PPC steps compare against a stable reference:

```bash
cargo bench -p gf2-core --bench <kernel-bench> -- --save-baseline ppc-v0-2026-04-25
```

Commit a small JSON manifest at `dev/benchmarks/ppc-baselines.json` mapping each Tier-A–D kernel to its baseline ID and the commit it was measured at. The committed `target/criterion/` directory the survey found is recent (Apr 18–25) but not version-pinned.

### I2 — Comparison harness

Add `dev/benchmarks/ppc-compare.sh` that runs the Tier-X bench against the pinned baseline and prints geomean speedup. The 1.5× gate is automated:

```
geomean(new/baseline) >= 1.5  → PASS
                       <  1.5  → FAIL (do not merge this spiral step)
```

### I3 — ASM-inspection convention

Each kernel directory gets a sibling `*.asm.txt` artefact regenerated via `cargo show-asm -p gf2-kernels-simd <symbol>` after every spiral step, so review can confirm e.g. `vpxor`, `vpclmulqdq`, `vpternlogq`, `vpermq` mnemonics actually landed. Reviewer bounces the PR if the asm doesn't match the spiral step's claim.

### I4 — Property-test scaffolding

Each new SIMD kernel ships with a proptest pair: scalar-vs-SIMD equivalence (random inputs, all sizes 0..1024, all alignments) and identity invariants for the field. These run as part of the fast-tier nextest suite. Word-boundary cases 0/1/63/64/65 are explicit `#[test]`s per `CLAUDE.md`.

### I5 — JIT issue tree (created in Phase 1, before any code)

One epic with five stories and ~22 tasks. Lead creates these via `mcp__jit__jit_issue_create` (or the `jit-manage` skill), wires dependency edges via `mcp__jit__jit_dep_add`, and attaches this plan file as the epic's spec doc via `mcp__jit__jit_doc_add`.

**Epic** — `feat: gf2-core PPC-spiral performance sweep`
- labels: `type:epic`, `epic:gf2-core-ppc-spiral`, `component:gf2-core`, `component:gf2-kernels-simd`
- spec doc: `/home/vkaskivuo/.claude/plans/analyze-the-core-library-sprightly-sprout.md`
- success criteria (epic-level, all `[hard]`): every Tier A–D kernel ships with criterion bench, asm artefact, proptest equivalence, and ≥1.5× geomean over the pinned baseline; clippy clean; fast-tier nextest green; MSRV not bumped without explicit user approval.

**Stories** (one per tier; each has `type:story`, `story:gf2-core-ppc-spiral-<tier>`):

| Story | Title | Children |
|---|---|---|
| S0 | `chore: PPC-spiral measurement infrastructure` | I1, I2, I3, I4 |
| SA | `perf: route scalar callers through existing SIMD dispatch` | A1, A2, A3 |
| SB | `perf: dense bit-matrix SIMD kernels` | B1, B2, B3 |
| SC | `perf: field-arithmetic batch kernels` | C1, C2, C3, C4, C5 |
| SD | `perf: sparse matvec layout + prefetch` | D1, D2 |
| SE | `perf: multithread scaled kernels` | E1, E2 |

**Tasks** — 22 total; each is `type:task` with `epic:gf2-core-ppc-spiral` and the parent `story:*` label. Each task description embeds the kernel's "Current / Fix / PPC steps" cell from the tier table above and the per-kernel execution protocol (1–7) verbatim. Each task carries:
- `[hard]` correctness: scalar-vs-SIMD proptest equivalence + word-boundary `#[test]`s pass.
- `[hard]` baseline saved + asm artefact present + perf-stat capture.
- `[aspirational]` 1.5× geomean (downgraded to aspirational only because the threshold may be empirically unreachable for cache-bound steps; falling below the bar means *do not merge that step*, not failure of the kernel — see protocol step 5).

**Dependency edges** (DAG; created via `jit_dep_add B blocks A` semantics):

- `I1, I2, I3, I4` (children of S0) block every task in SA, SB, SC, SD.
- All tasks within SA are independent (parallel-safe, worktrees required).
- `B1` (transpose) blocks `B3` (tile-reuse asm path reuses transpose primitive).
- `C4` (QuadraticExt SoA) blocks `C5` (CubicExt reuses SoA scaffolding).
- `C3` carries an explicit gate: AVX2 path lands first; the IFMA52/AVX-512 sub-task is dependent and only opened if the AVX2 result is below 1.5× — and even then, escalation per `escalation-policy.md` is required before the MSRV-bump dependent task is dispatched.
- `E1` blocks on all of SB; `E2` blocks on `C4, C5`.

**Gates** registered on every task via `mcp__jit__jit_gate_add`:
- `cargo-fmt` (project-standard)
- `cargo-clippy` (workspace, all-targets, -D warnings)
- `cargo-nextest-fast` (workspace, --release, --profile ci)
- `criterion-1.5x` (custom, runs `dev/benchmarks/ppc-compare.sh <kernel-id>` and asserts exit 0)
- `asm-artefact-present` (custom, asserts the sibling `*.asm.txt` was regenerated this commit)
- `code-review` (per-PR, runs the project's reviewer prompt)

The epic itself carries an additional `coherence-review` gate that the lead runs before the epic is marked done — verifies that the dispatch boundary stays clean, no `unsafe` leaked outside `gf2-kernels-simd`, and that all six per-kernel deliverables are present for every Tier A–D kernel.

---

## Per-kernel execution protocol (PPC always-do)

For every kernel ID above, the spiral step that ships must satisfy all of:

1. **Baseline saved** before the first edit (`--save-baseline`).
2. **Asm inspected** — sibling `.asm.txt` updated; reviewer verifies the expected mnemonic appears.
3. **`perf stat -r 10`** captured before and after; deltas in IPC, L1d-misses, branch-misses recorded in the JIT issue. (Cache-bound steps without IPC delta are still valid if cache-miss rate drops materially.)
4. **Correctness**: scalar-vs-SIMD proptest equivalence (I4) plus all existing tests stay green at fast tier.
5. **Speedup**: `geomean(new/baseline) >= 1.5` on the kernel's design size class. Below the bar → revert and try a different transform; do not merge "small wins."
6. **One commit per spiral step**, scope-prefixed `perf(jit:<short-id>): Vk — <transform>`, per `CLAUDE.md` git workflow.
7. Lead-only `jit gate pass/fail` per memory `feedback_agents_no_gate_runs.md`. Workers commit + return; lead transitions state.

---

## Critical files to touch (reference, not exhaustive)

- Dispatch boundary: `crates/gf2-core/src/lib.rs` (add `maybe_transpose`, `maybe_gf2m_batch`, etc., next to existing `maybe_simd`, `maybe_gf2m`, `maybe_mersenne`, `maybe_fp65537`, `maybe_gf2m_wide256`).
- New SIMD impls: `crates/gf2-kernels-simd/src/x86/{transpose.rs, gf2m_batch.rs, fp_general.rs}` and `crates/gf2-kernels-simd/src/{transpose.rs, gf2m_batch.rs, fp_general.rs}` (safe wrappers).
- Detection registration: `crates/gf2-kernels-simd/src/x86/mod.rs` `detect_x86()` extension.
- Kernel callers: `crates/gf2-core/src/matrix.rs`, `crates/gf2-core/src/sparse.rs`, `crates/gf2-core/src/gf2m/field.rs`, `crates/gf2-core/src/gfp/simd_ops.rs`, `crates/gf2-core/src/gfpn/{quadratic.rs,cubic.rs}`, `crates/gf2-core/src/alg/m4rm.rs`.
- Benches that already cover the targets and need new variants for "after" measurement: `crates/gf2-core/benches/{matrix_vector.rs, matmul.rs, sparse.rs, gf2m_mul_strategies.rs, gf2m_wide_mul.rs, fp_specialized.rs, fieldvec_dot_product.rs, field_matrix.rs, field_matrix_gemm.rs, field_matrix_fusion.rs}`.
- Compute parallel layer: `crates/gf2-core/src/compute/cpu.rs` (Tier E only).

## Hand-off to project-lead

After Phase 1 (issue scaffolding) is complete, the lead invokes:

```
/project-lead <epic-short-id>
```

The `project-lead` skill drives the epic to completion: breaks down stories further if needed via `jit-breakdown`, dispatches per-task workers (in worktrees for parallel-safe tasks), runs holistic coherence review per the project's escalation policy, and bounces work that fails any of the six gates above. The lead — not the workers — performs all `jit gate pass/fail` transitions.

## Verification (end-to-end)

For each Tier letter:

1. `cargo nextest run -p gf2-core --release --profile ci` passes (fast tier, 5s/test cap).
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
3. `cargo bench -p gf2-core --bench <kernel-bench> -- --baseline ppc-v0-2026-04-25` shows ≥1.5× geomean on the kernel's design size class.
4. `dev/benchmarks/ppc-compare.sh <kernel-id>` exits 0.
5. `cargo show-asm` artefacts match the claimed transform.
6. JIT gate `code-review` passes on the per-kernel issue.

If a Tier-C MSRV-bump candidate (C3 IFMA52) actually demonstrates the speedup, the MSRV bump itself is a separate user-approval-gated escalation per `CLAUDE.md` "Verification work" and `escalation-policy.md`.

# Benchmark-gap strategy for `64c88ae4` in the PPC sweep

**Issue:** `085cbd0b`  
**Epic:** `babcf05e` — gf2-core PPC-spiral performance sweep  
**Scope:** strategy only. No implementation, no new benchmark runs, no JIT state transitions.

## Problem statement

The original PPC Tier A-D plan is necessary because it sharpens the existing hot kernels: dispatch hoisting, row-XOR routing, M4RM table/block work, GF(2^m) element batches, general-prime Montgomery batches, extension-field SoA, and sparse layout. It is not sufficient for state-of-the-art performance because the `64c88ae4` report exposes algorithmic gaps above those kernels:

- `fgemm × GF(2^31-1)` reaches only `0.25×` of fflas-ffpack at `n=1024` (`590 Mops/s` vs `2.34 Gops/s`), after being closer at `n=64` (`0.44×`). The report attributes the widening gap to fflas-ffpack's Strassen/BLAS cascade while gf2 remains schoolbook above the crossover.
- `matmul × GF(2)` reaches only `0.13×` of M4RI at `n=1024` (`387 Gops/s` vs `3.02 Tops/s`) and `0.17×` at `n=4096`. The uncompetitiveness profile captured the pre-`59c487c3` state: M4RI switched to Strassen-Winograd at large sizes while gf2 used pure M4RM. Post-`59c487c3`, gf2 has a test-support Strassen-family layer, but `dev/bench_results/2026-04-29-strassen-matmul-crossover.md` found no measured crossover through `8192`; production dispatch therefore remains on M4RM for all real workloads until a winning crossover is demonstrated.
- GF(2^m) FieldMatrix multiply is `~16-18×` slower than the Mersenne-31 Fp path at every measured size: e.g. `GF(2^16)` fgemm is `32.0 Mops/s` at `n=1024` vs `589.6 Mops/s` for `Fp(2^31-1)`. The report identifies missing PCLMUL-backed FieldMatrix dispatch, not just missing scalar cleanup.

Therefore Tier A-D should remain the kernel foundation, while the new `3abb755e` story adds matrix-level algorithms that consume those kernels and avoid duplicating them.

## Gap-to-task mapping

| Gap from `64c88ae4` | Evidence | Responsible JIT task(s) | Dependency / wave relation | Non-goals |
|---|---|---|---|---|
| GF(2) BitMatrix multiply is far behind M4RI at large `n`. | `dev/bench_results/2026-04-26.md` § `matmul × GF(2)`: `n=1024` ratio `0.13×`; `n=4096` ratio `0.17×`. `dev/bench_results/2026-04-26-uncompetitiveness-profile.md` § TL;DR / Numbers captured the pre-`59c487c3` state: SIMD alone gave only `+16%` at `n=1024`, `+24%` at `n=4096`, with no Strassen layer. `dev/bench_results/2026-04-29-strassen-matmul-crossover.md` supersedes that implementation-state claim: the Strassen-family path now exists for correctness and benchmark forcing, but forced Strassen did not beat M4RM through `8192`, so production auto dispatch stays on M4RM for all sizes pending a measured crossover. | `59c487c3` (Strassen-Winograd layer), after `19bc3199` (B3 M4RM block multiply). Supporting Tier A/B tasks: `c69d2055`, `5223bb04`, `54a0e75c`, `19bc3199`. | Strategy `085cbd0b` gated `59c487c3`; `19bc3199` had to land first so Strassen leaves call the optimized M4RM backend. Tier A dispatch hoist reduces per-leaf overhead but is not the Strassen implementation. | Do not rewrite M4RM inside `59c487c3`; do not remove scalar/M4RM fallback paths; do not add a new public matrix type solely for Strassen. |
| GF(p) FieldMatrix multiply is behind fflas-ffpack and falls further behind with size. | `dev/bench_results/2026-04-26.md` § aspirational target status: `GF(2^31-1)` fgemm `n=1024` ratio `0.25×`; PLUQ `n=256` ratio `0.34×`; gap opens after `n=64` (`0.44×` fgemm, `0.92×` PLUQ). | `e7ab802d` (delayed-reduction blocked FieldMatrix multiply), after `86c09a51` (C3 general-prime Montgomery batch SIMD). Supporting Tier A/C tasks: `cad241e6`, `86c09a51`; extension SoA work `3168d114` / `33d3f5b7` may later reuse the blocked schedule. | Strategy `085cbd0b` gates `e7ab802d`; C3 must supply a measured SIMD Montgomery batch primitive first. Delayed reduction is a matrix/block algorithm above C3, with its own accumulator-bound proof/measurement. | Do not replace C3's element-kernel work; do not assume fflas-ffpack's floating/modular trick is directly legal for all `Fp<P>`; do not change shared `Field` / `FieldMatrix` APIs without escalation. |
| GF(2^m) FieldMatrix multiply is much slower than the optimized Fp path and has no reference-side fflas row yet. | `dev/bench_results/2026-04-26.md` § GF(2^m) gap: `GF(2^8)`/`GF(2^16)` fgemm are `~32-36 Mops/s` vs `~590 Mops/s` for Mersenne-31; `~16-18×` slower. Reference fflas GF(2^m) rows are `PENDING`, so current evidence is gf2 internal and absolute throughput. | `577b9e7f` (wire GF(2^m) batch kernels into FieldMatrix multiply), after `ec286cee` (C1 m=8/16/32 VPCLMUL batch). Related but separate: `7c954fb5` (C2 m=571). | Strategy `085cbd0b` gates `577b9e7f`; C1 owns the element-wise VPCLMUL kernel. `577b9e7f` owns matrix tiling/dispatch so FieldMatrix actually uses C1 in bulk. | Do not duplicate C1's PCLMUL reduction kernel; do not block on fflas GF(2^m) harness extension; do not optimize m=571 in this task unless it naturally falls out of the C2 path. |
| Low-level SIMD dispatch overhead can hide kernel wins. | `dev/bench_results/2026-04-26-uncompetitiveness-profile.md` § Evidence B and `dev/bench_results/2026-04-27-asm-audit.md` § LTO-opacity: all five dispatch tables remain opaque through `OnceLock` + fn pointers. | Existing Tier A task `c69d2055`; analogous hoisting may be needed inside `59c487c3`, `e7ab802d`, and `577b9e7f` loops when they call dispatch-backed kernels repeatedly. | Tier A precedes algorithmic tasks conceptually; algorithmic workers should bind resolved kernel functions/tables outside inner loops when the dependency already exposes that pattern. | Do not rely on ThinLTO to inline through dispatch tables; do not introduce new un-hoisted `maybe_*()` probes in hot loops. |

## Composition plan

### GF(2): Strassen-Winograd above M4RM

`59c487c3` composes **above** Tier B. Its recursive leaves call the best available M4RM/block multiply path from `19bc3199`; below measured crossover it must preserve the existing M4RM/scalar fallback selection. The public API should remain the existing `BitMatrix` / `alg::m4rm` surface unless measurement proves a shared interface change is required, in which case escalate before changing it. Recursion thresholds, leaf sizes, temporary allocation strategy, and rectangular gating are local measured decisions.

### GF(p): delayed reduction above C3 Montgomery batches

`e7ab802d` composes **above** `86c09a51`: C3 provides measured SIMD Montgomery batch operations for general primes; delayed reduction provides a blocked FieldMatrix schedule that accumulates several multiply-adds before reducing. This task needs an explicit accumulator-bound design per supported prime class and block size. If accumulator bounds force changes to shared `Field`, `FieldMatrix`, `SimdVecOps`, or public constructors, that is escalation-worthy before implementation proceeds.

### GF(2^m): FieldMatrix tiling above C1 element kernels

`577b9e7f` composes **above** `ec286cee`: C1 owns the VPCLMUL element/batch kernels for m=8/16/32; the new task owns FieldMatrix-level tiling, chunking, and dispatch so matrix multiply stops calling scalar `Gf2mWide` arithmetic in the inner loop. It should reuse C1's kernel entrypoints and scalar fallback, not fork the reduction code. C2 (`7c954fb5`) remains the separate path for large m such as 571.

### Tier D/E interaction

Sparse Tier D and multithread Tier E are consumers of evidence, not prerequisites for the three new algorithmic tasks. Tier E should still wait until single-thread cache and kernel behavior are measured; do not mask a poor single-thread algorithm with rayon. If Strassen or FieldMatrix tiling changes cache-miss shape, feed those measurements into Tier E scheduling rather than opening a parallel implementation immediately.

## Decision policy

### Workers may decide locally by measurement

- Crossover thresholds for Strassen leaves, M4RM/scalar fallback, and rectangular shape gating.
- Tile sizes, cache-block sizes, row/column panel shapes, and prefetch distances.
- Whether to revert a transform that fails the issue's benchmark threshold.
- Which existing fallback path to use below crossover when the public API is unchanged.
- Benchmark-size additions needed to exercise a new threshold, if they stay inside the issue scope and use release-mode fast-tier conventions.

### Escalate to the user / project lead first

- Public API changes to `BitMatrix`, `FieldMatrix`, `Field`, `SimdVecOps`, or shared dispatch infrastructure.
- Changes to issue success criteria, especially `[hard]` performance or correctness requirements.
- MSRV changes, new hardware-feature scope, or requiring AVX-512/ROCm where the issue did not already say so.
- Accepting a result below a hard performance bar or changing the meaning of the criterion after measurement.
- Changes to shared infrastructure such as gate scripts, baseline manifests, dispatch-table layout, or cross-issue benchmark conventions.
- Accumulator-bound assumptions for delayed reduction if they restrict supported primes or require a new trait/interface contract.

## Dispatch guidance and traps

- **Measurements, not guesses.** Run a pre-dispatch criterion audit for hard performance claims; if a number was a hypothesis, treat it as suspect until measured.
- **Do not rely on the falsified ThinLTO premise in `c69d2055`.** The useful lever is hoisting dispatch and removing `OnceLock` probes/branches from hot loops. The asm audit confirms the dispatch tables remain opaque.
- **Lead runs JIT gates.** Workers must not run gate shell scripts directly and must not call `jit gate pass`, `jit gate fail`, `jit issue update`, or other state-transition commands.
- **Existing untracked file:** `dev/bench_results/2026-04-26-gf2-simd.csv` may be present in the working tree. It is unrelated to this strategy note; do not commit, modify, or delete it.
- **Release mode only for tests/benches.** Do not run slow ignored tests as an agent. Use the existing cargo/nextest/criterion workflows.
- **Unsafe isolation stays intact.** New unsafe SIMD belongs only in accelerator kernel crates. High-level algorithmic code should call safe wrappers and preserve scalar fallbacks.
- **Avoid duplicated work.** Tier C1/C3 implement element kernels; `577b9e7f`/`e7ab802d` implement matrix schedules. Tier B3 implements M4RM leaf performance; `59c487c3` implements recursion above it.

## Non-goals for this note

- No implementation or source-code changes.
- No new benchmark runs or new baseline pinning.
- No JIT state transitions or gate mutations.
- No doc linking; the project lead will attach this document to `085cbd0b`, `59c487c3`, `e7ab802d`, and `577b9e7f` with `jit doc add`.

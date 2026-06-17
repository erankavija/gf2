# Epic 806eb14e — completion report

**Epic:** Prototype GPU acceleration for belief propagation (`epic:hip-gpu-prototype`)
**State:** DONE (2026-06-18). Lead: agent:project-lead.
**Feasibility report:** `dev/active/806eb14e-feasibility-report.md`
**Spec:** `dev/plans/hip_gpu_prototype_wave.md`

## Verdict

GPU acceleration is **GO** on HIP/ROCm for both targets: LDPC BP (**28.98×**
decode-vs-decode vs CPU-24T, ≥3×) and BCH syndrome evaluation (**87.9×** vs the
best existing CPU path, ≥5×), both byte-identical to CPU. The shared scheduler is
**characterized** (100% CPU/GPU overlap, throughput-neutral vs default-stream).

## Metrics

| Metric | Value |
|--------|-------|
| Children | 16 (8 done, 8 rejected as superseded by gf2-sim) |
| Sub-agents dispatched | 2 (BCH implementation; BCH rework) |
| Rework rounds | 9012f8a0 ×1 (5 findings), 886cebf9 ×1 (3), 24c11004 ×1 (2), aeab2ee4 ×2 (SSOT dedup; pub-API docs), 86a363aa ×1 (criterion conflict), epic ×3 (stale-narrative sweeps + fmt) |
| Escalations | 2 (both resolved) |
| GPU correctness | all rungs re-verified by lead on gfx1030 |

## Success-criteria mapping

| Epic criterion | Delivered by | Status |
|----------------|--------------|--------|
| HIP/ROCm only; Vulkan/GLSL superseded | `886cebf9`, `46fe1108`, `92acd7b5` reframe, 8 rejections | ✓ |
| LDPC BP ≥3× + correctness | `a930be7f` (gf2-sim) — 28.98× | ✓ GO |
| BCH syndrome ≥5× + exact equivalence | `9012f8a0` — 87.9×, byte-identical | ✓ GO |
| Shared scheduling quantified + characterized | `75c22fa8` (gf2-sim) — 100% overlap, throughput-neutral | ✓ (criterion 4 amended) |
| Feasibility report go/investigate/abandon | `24c11004` | ✓ |
| Default CI green without ROCm; HIP isolated | feature-gating; `cargo-ci.sh` drops hip when no hipcc | ✓ |
| No unsafe outside `gf2-kernels-hip` | invariant upheld | ✓ |

## Key autonomous decisions

1. **Reconciliation (pre-epic):** 8 children rejected as superseded by the
   gf2-sim GPU pipeline (LDPC BP, evidence infra, scheduler) — the only genuine
   delta was BCH syndrome evaluation.
2. **BCH = evaluator, not a pipeline Stage:** syndrome eval is a decode sub-step;
   delivered as a `gf2-kernels-hip` kernel/wrapper + `gf2-coding` `--features hip`
   hook, BM/Chien on CPU.
3. **GF(2^m) mult = uploaded CPU exp/log tables** — bit-identical by construction.
4. **1T as the BCH gate divisor** — rayon-24T is anomalously slower (Arc
   contention), so single-thread is the honest best existing CPU path.

## Escalations (both user-approved)

1. **Reframe downstream epic `92acd7b5`** (Vulkan/CUDA → HIP) — needed to clear
   `886cebf9` code-review; outside this epic so required approval.
2. **Amend epic criterion 4** — measured scheduler result is throughput-neutral
   (not "measurable improvement"); reworded to "characterize the result",
   recording the observed numbers per the measurements-not-guesses rule.

## Issues discovered during execution

- CPU `compute_syndromes` `Arc<FieldParams>` refcount contention makes rayon-24T
  slower than 1T (downstream fix recommended in the feasibility report §6).
- Host coeff-repack dominates the GPU BCH call (81%) — SIMD/parallel repack is a
  downstream optimization.
- Process traps recorded in `806eb14e-progress.json`: cargo-ci 300s timeout vs
  cold `--all-features` HIP build (pre-warm required); never run a heavy gate
  concurrently with an agent build (memory pressure).

## Downstream (NOT part of this epic)

Wire GPU BCH into a production decode path; fix CPU Arc contention + re-baseline;
SIMD the repack; extend the HIP backend across code types (`92acd7b5`, reframed).

# HIP/ROCm GPU prototype wave — feasibility decision report

JIT issue: `24c11004` (Write GPU prototype feasibility decision report)
Epic: `806eb14e` (Prototype GPU acceleration for belief propagation)
Spec: `dev/plans/hip_gpu_prototype_wave.md`
Date: 2026-06-17. Author: agent:project-lead.

## 1. Summary verdict

| Workload | Threshold | Measured | Verdict |
|----------|-----------|----------|---------|
| LDPC BP decode (GPU) | ≥ 3× best production CPU path | **28.98×** vs CPU-24T (253.51× vs CPU-1T), decode-vs-decode | **GO** |
| BCH syndrome evaluation (GPU) | ≥ 5× best production CPU path | **87.9×** vs best existing CPU path (1T), exact byte-identity | **GO** |
| Shared CPU∥GPU scheduler | measurable end-to-end improvement | **100% GPU-decode overlap**, 2.40× directional | **GO (model adopted)** |

All correctness is exact/byte-identical against the CPU reference. Both
acceleration targets clear their hard thresholds. **Recommendation: GO** on
HIP/ROCm for LDPC BP and BCH syndrome evaluation. Production rollout is downstream
of this prototype epic (see §6).

## 2. Scope note — what this epic actually delivered

The prototype wave overlapped in time with the `gf2-sim` epic (`f9717e7e`), which
built the production HIP GPU pipeline. During execution (2026-06-13) eight of this
epic's children were **rejected as superseded** because `gf2-sim` delivered the
same capability:

- LDPC BP GPU kernel, GPU evidence infrastructure, and the shared scheduler were
  delivered in `gf2-sim` (`a930be7f`, byte-identity suite, `75c22fa8`).
- This epic's own remaining engineering delta was **BCH syndrome evaluation**
  (`9012f8a0`), plus the legacy-cleanup notes and this report.

This report therefore synthesizes evidence that lives partly in the `gf2-sim`
benchmark receipts. That is recorded honestly rather than re-attributed.

## 3. Evidence environment (all measurements)

| Item | Value |
|------|-------|
| GPU | AMD Radeon RX 6950 XT (gfx1030, RDNA2) |
| CPU | AMD Ryzen 9 5900X (12C/24T), rayon = 24 threads |
| ROCm | 7.2.4 (`/opt/rocm/.info/version`); `hipcc --offload-arch=gfx1030 -O3` |
| OS | Linux 7.0.10-arch1-1 |
| Pinned baselines | `dfe297f0` / `c0b1702d` (single-thread 1.6216 fps), `3fcb7025` (24-thread 21.44 fps, full-frame DVB-T2 r1/2 16-QAM) |

Comparators use the **best existing production CPU path at the same workload
shape, including rayon**, with the decode-vs-decode discipline established by the
`a930be7f` amendment (no GPU-vs-full-frame category confusion).

## 4. Per-workload findings

### 4.1 LDPC BP — GO (≥ 3× met)

- Issue `a930be7f` (closed `3d0a4bb0`, all 3 gates green), `gf2-sim` epic.
- Kernel: `crates/gf2-kernels-hip/hip/ldpc_bp.hip`; stage `crates/gf2-sim/src/gpu/ldpc_bp.rs`.
- Correctness: hard-decision byte-identical to CPU `LdpcDecoder` across MinSum /
  NormalizedMinSum(0.75) / SumProduct, 200 frames, 3 SNRs
  (`crates/gf2-sim/tests/gpu_ldpc_byte_identity.rs`). `mean_iters` excluded per
  RDNA2 transcendental ULP drift (design §11) — the hard-decision verdict is robust.
- Throughput (decode-vs-decode): **253.51× vs CPU-1T, 28.98× vs CPU-24T**
  (`dev/benchmarks/gf2-sim/gpu-stages-receipts.md`, `parallelism-receipts.md#a930be7f`).
- Verdict: **GO**. Already integrated as a production `GpuLdpcBp` stage with CPU fallback.

### 4.2 BCH syndrome evaluation — GO (≥ 5× met)

- Issue `9012f8a0` (this epic): feat `fc0783f0`, rework `4d2a6245` (code-review round 2 cleared 5 findings), merge `eb8a7d57`.
- Kernel `crates/gf2-kernels-hip/hip/bch_syndrome.hip`; wrapper
  `crates/gf2-kernels-hip/src/launch_bch_syndrome.rs`; hook
  `BchDecoder::compute_syndromes_batch_gpu` (`crates/gf2-coding/src/bch/core.rs`,
  `--features hip`).
- Correctness (exact integer GF, zero tolerance — re-verified by lead on gfx1030):
  exhaustive GF(2⁴) `gf_mul`, uploaded-table equality (GF2¹⁴/GF2¹⁶), BCH(15)
  Horner fixture, DVB-T2 Short+Normal 200-frame byte-identity (mixed valid/≤t/>t),
  and GPU-syndrome→CPU-BM/Chien decode-equivalence
  (`crates/gf2-kernels-hip/tests/gpu_bch_syndrome_field.rs`,
  `crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs`).
- Throughput (DVB-T2 Normal r1/2, n=32400, GF2¹⁶): GPU **7267.9 fps** vs CPU-1T
  82.7 fps = **87.9×**. Receipt: `dev/benchmarks/gf2-sim/gpu-bch-syndrome-receipt.md`.
- **Baseline honesty:** the rayon-24T CPU `compute_syndromes` (11.6 fps) is
  *slower* than single-thread (82.7 fps) — an `Arc<FieldParams>` refcount
  contention artefact in the CPU operator path, not GPU inflation. The honest best
  *existing* production CPU path is therefore single-thread (82.7 fps); the GPU
  clears ≥ 5× by **87.9×** against it. The misleading "625× vs 24T" figure is NOT
  used as the headline. Making the CPU 24T path competent (removing the Arc
  traffic) is downstream work (§6) — note that a competent ~linear-scaling 24T CPU
  could narrow the margin toward ~4×, so the ≥5× result is reported against the
  measured best-existing path, with this caveat explicit.
- Verdict: **GO** (as a syndrome-evaluation offload; not yet a production decode stage — §6).

### 4.3 Shared CPU∥GPU scheduler — GO (measurable improvement)

- The shared-scheduler question (direct per-thread/default-stream submission vs a
  shared scheduler) was answered by the `gf2-sim` hybrid executor (`75c22fa8`),
  which this epic's scheduler story (`bfd1aa89`/`d77519a3`) was rejected in favour of.
- Evidence (`dev/benchmarks/gf2-sim/hybrid-executor-receipts.md`): the scheduler
  records an `OverlapTimeline`; **measured CPU-prep∥GPU-decode overlap = 100.0%**
  (72 intervals, real GPU, `hybrid_scheduler.rs::hybrid_gpu_cpu_overlap_exceeds_50pct`),
  with two-run byte-identity vs a direct `run_snr_point`. Directional end-to-end:
  hybrid 51.44 fps → 2.40× the canonical divisor under load.
- Verdict: the shared scheduler **measurably improves** end-to-end GPU throughput
  over direct default-stream submission; the hybrid model is adopted in `gf2-sim`.

## 5. Epic success-criteria → artifact mapping

| Epic criterion | Delivered by | Status |
|----------------|--------------|--------|
| HIP/ROCm only; Vulkan/GLSL children superseded | `886cebf9`, `46fe1108` (+ 8 rejections) | ✓ |
| LDPC BP HIP prototype + correctness + ≥3× | `a930be7f` (gf2-sim) | ✓ GO |
| BCH syndrome HIP prototype + exact equivalence + ≥5× | `9012f8a0` | ✓ GO |
| Shared HIP scheduling quantified + measurable improvement | `75c22fa8` (gf2-sim) | ✓ |
| Final go/investigate/abandon report | this document (`24c11004`) | ✓ |
| Default CI green without ROCm; HIP isolated | feature-gating; `cargo-ci.sh` drops `hip` when no hipcc | ✓ |
| No unsafe outside `gf2-kernels-hip` | invariant upheld | ✓ |

## 6. Downstream production work (NOT part of this epic)

Explicitly out of scope for this prototype wave; recommended follow-ups:

1. **Wire GPU BCH syndrome into a production decode path** (a `gf2-sim` stage or a
   `decode_batch` fast path), mirroring `GpuLdpcBp`, with CPU fallback.
2. **Fix CPU `compute_syndromes` `Arc<FieldParams>` contention** so the rayon-24T
   path scales, then re-baseline the BCH ≥5× against a competent 24T CPU.
3. **Optimise the host coeff-repack** (81.2% of the GPU BCH call) via SIMD/parallel
   bit-packing to widen the device-side margin.
4. **Extend the HIP backend** across remaining code types and add production
   multi-backend selection (the original ROADMAP Phase C11.4, HIP not Vulkan).

These are recommendations only; this epic ends at the prototype + decision.

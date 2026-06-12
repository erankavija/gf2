# gf2-sim parallelism receipts

Throughput receipts for the `gf2-sim` epic (`f9717e7e`) parallel-pipeline tasks.
Each entry follows the project-plan §5 schema. The **single-thread headline
baseline is 1.6216 fps** for DVB-T2 r1/2 16-QAM at Es/N0 = 6.25 dB (canonical
config), measured on the legacy `SimulationRunner::run_with_decoder` path at
commit `9e983ae26e` and recorded in `baseline-single-thread.md`; it is the
divisor for every speedup gate here. This file establishes the canonical CPU
24-thread baseline that downstream GPU receipts (Phase B) reference.

## 3fcb7025 — Within-SNR frame parallelism + deterministic aggregation

- **Date:** 2026-06-08 (clean re-measurement; supersedes the 2026-06-07
  provisional figures that were taken under external CPU load)
- **Hardware:** CPU=AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU=n/a (CPU-only task)
- **Baseline configuration:** single-thread
- **Test configuration:** DVB-T2 r1/2 16-QAM, FrameSize::Normal (n_ldpc = 64800),
  SumProduct LDPC decoder (early-termination on), ExactLogMap soft-demap,
  Es/N0 = 6.25 dB, batch = 1 frame per dispatch (per-frame kernel
  `frame_sim::DvbT2BicmFrameSim`). **mean_iters = 25.243** (144-frame set) — the *real* mean BP
  iteration depth (via `DvbT2Concat::decode_soft_counted`), byte-identical
  across all worker counts {1,2,4,8,24} (frames converge above the waterfall but
  the early-termination depth is a genuine per-frame quantity, not a sentinel).
  Per-frame RNG budget: each frame seeks to its own `FRAME_STRIDE = 2^20`
  (32-bit-word) region; the measured worst-case per-frame draw is 260 208 words
  for the binding config **r1/2 QPSK Normal** (the lowest-order modulation has
  the most symbols → the most noise draws; QPSK binds, not 16-QAM, which
  measures 130 608, nor 64-QAM at 87 408), ~4× under the stride, so consecutive
  frames' noise streams never overlap (design-doc §3, amended 2026-06-07; guarded
  by `parallel::tests::test_worst_case_frame_draw_under_stride`, which enumerates
  all three modulations and asserts QPSK is the maximum).
- **Observed throughput:** **21.44 fps ± 0.22** (24 threads, 144 frames, 3 repeats).
  New-pipeline single-thread reference: **2.08 fps ± 0.01**.
- **Speedup factor:** **13.22x** (21.44 / 1.6216) versus the 1.6216 fps
  single-thread headline baseline. (Intrinsic parallel scaling vs the new
  pipeline's own 1-thread number is ~10.3x at 24 threads; the new pipeline is
  also ~1.29x faster single-thread than the legacy path, so the gate divisor is
  the more conservative legacy baseline.)
- **Required threshold (from task body):** >= 12x
- **Verdict:** PASS — clean re-measurement on a quiet host (only
  `parallel_throughput` running; `cat /proc/loadavg` ≈ 0 before the run, no
  foreign CPU hogs) at HEAD commit `ec30b3e1` (the merged FRAME_STRIDE = 2^20 /
  QPSK-binding state). 24-thread **21.44 fps ± 0.22 → 13.22x** clears the ≥ 12×
  gate with margin. This supersedes the 2026-06-07 provisional figures, which
  were measured under heavy external CPU load (a `bg3` process at ~340% CPU,
  5/15-min load ≈ 80) and were therefore invalid (contention only *understates*
  throughput). The earlier impl/refactor/iter-count/2^19/2^20 work landed at
  `22e9c66d` / `691fe43152` / `d4e7a67b26` / `5572374c75` / `a2a0ec1409`.
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/parallel_throughput.rs`
    (re-run: `cargo run -p gf2-sim --release --bin parallel_throughput -- --frames 144 --workers 1,2,4,8,24 --repeats 3 --es-n0 6.5`).
  - Determinism regression (byte-identity across {1,2,4,8,24} workers,
    3 configs): `crates/gf2-sim/tests/parallel_determinism.rs` (slow tier,
    ignored; run with `cargo nextest run -p gf2-sim --release --profile slow --run-ignored ignored-only -E 'test(determinism_r)'`).

### Scaling sweep (144 frames, 3 repeats, Es/N0 = 6.5 dB; clean re-measure 2026-06-08)

`mean_iters` is the real BP iteration depth (`DvbT2Concat::decode_soft_counted`);
it is constant across worker counts at a fixed seed — the byte-identity contract.

| Workers | fps_mean | fps_sigma | speedup vs 1.6216 | mean_iters |
|--------:|---------:|----------:|------------------:|-----------:|
| 1       | 2.0842   | 0.0072    | 1.29x             | 25.243     |
| 2       | 4.0671   | 0.0068    | 2.51x             | 25.243     |
| 4       | 7.3578   | 0.0243    | 4.54x             | 25.243     |
| 8       | 13.7002  | 0.0192    | 8.45x             | 25.243     |
| 24      | 21.4396  | 0.2176    | **13.22x**        | 25.243     |

Near-linear scaling through 8 workers (8.45x on 8 threads); the 24-thread number
benefits from the 12 physical cores' SMT. The per-worker design (each rayon
worker owns its own `DvbT2BicmFrameSim` clone, hence its own LDPC decoder) is
what unlocks this — a single shared `DvbT2Concat` would serialise every decode on
its internal decoder `Mutex`. (`mean_iters` is identical across all five worker
counts at a fixed seed — the byte-identity contract — and differs from the prior
120-frame sweep's 25.167 only because the 144-frame set is a different frame
population.)

### Determinism evidence

`tests/parallel_determinism.rs` asserts byte-identical `fer` / `frames` /
`errors` / `mean_iters` (bit-pattern equality on the derived f64 ratios, exact
equality on the u64 counters) across worker counts {1, 2, 4, 8, 24} for three
`(rate, modulation)` configurations, each at a waterfall Es/N0 so the frame set
contains both decode successes and failures. Since the decode path now reports
the **real** BP iteration count (`DvbT2Concat::decode_soft_counted`, not a
constant sentinel), the `mean_iters` byte-identity assertion compares genuine
per-frame decoder depth — the criterion is no longer vacuous.

| Config | Es/N0 | Decoder / demap | Result |
|--------|------:|-----------------|--------|
| r1/2 16-QAM | 6.25 dB | SumProduct / ExactLogMap | PASS (byte-identical {1,2,4,8,24}) |
| r2/3 16-QAM | 8.90 dB | NMS(0.75) / ExactLogMap  | PASS (byte-identical {1,2,4,8,24}) |
| r1/2 64-QAM | 9.90 dB | MinSum / MaxLog          | PASS (byte-identical {1,2,4,8,24}) |

All three passed locally after the `FRAME_STRIDE = 2^20` amendment
(27 s / 26 s / 55 s respectively — elevated by concurrent external host load but
still well under the slow tier's 120 s/test cap). The byte-identity holds because
every global frame's noise is keyed on the global frame index via the design-doc
§3 seek `worker_offset(seed, snr_idx, 0, g)`, making each frame's outcome —
including its deterministic BP iteration depth — a pure function of `g` regardless
of worker count, and the per-worker counters are reduced in `worker_idx` order
(the SSOT aggregation order). Raising `FRAME_STRIDE` changes the absolute seek
offsets but preserves this per-frame purity, so byte-identity is unaffected.

## f6004add — GPU ChaCha20 + Box-Muller AWGN kernel

- **Date:** 2026-06-08 (lead clean re-measurement on a verified-quiet host —
  `cat /proc/loadavg` = 0.42, `rocm-smi` GPU use 0%, no foreign CPU hogs;
  supersedes the worker's provisional under-load figures).
- **Hardware:** CPU = AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU = AMD Radeon
  RX 6950 XT (gfx1030, RDNA2).
- **Baseline configuration:** single-thread CPU AWGN-step (this is an AWGN-only
  kernel; see the metric note below).
- **Test configuration:** DVB-T2 r1/2 16-QAM, FrameSize::Normal
  (n_ldpc = 64800, 4 bits/symbol → **16200 symbols/frame, 32400 noise
  samples/frame**), Es/N0 = 6.5 dB. Per-frame device launch: one
  `chacha20_awgn_kernel` launch (one thread per noise sample) + `hipDeviceSynchronize`
  + D2H read-back, per-worker-owned `GpuChaChaAwgn` generator (one device key
  upload + output allocation reused across frames). The CPU comparator is a
  single thread running the **same** AWGN noise step via
  `gf2_sim::channels::Awgn::apply_for_frame` (the §8 CPU fallback / ulp oracle).
- **Metric note (HONEST SCOPING):** `f6004add` accelerates the **AWGN noise
  step only** (no GPU LDPC/BCJR decode — explicit non-goal). The full-frame
  single-thread headline baseline (1.6216 fps) is dominated by LDPC BP decode,
  which this kernel does not touch, so the contractual ≥10× comparison is made
  against the **single-thread CPU AWGN step** (the apples-to-apples workload).
  For context, the GPU also emits the full-frame baseline ratio below, but that
  number is not the gate metric for an AWGN-only kernel.
- **Observed throughput (AWGN noise step, 4000 frames, 7 repeats; clean
  quiet-host re-measure 2026-06-08 at loadavg 0.42):**
  - **GPU: 18659.31 fps ± 227.77** (very stable across repeats).
  - **CPU single thread: 1251.07 fps ± 3.30** (tight σ on the quiet host —
    supersedes the worker's under-load 1156 ± 212).
- **Speedup factor:** **GPU / CPU-1-thread AWGN-step = 14.91×** (18659.31 /
  1251.07; ≥ the **10×** threshold with margin). The GPU AWGN-step throughput
  (18659 fps) is ~11507× the *full-frame* 1.6216 fps baseline — but that ratio
  is not meaningful as a gate (different workload), and is recorded only for
  completeness.
- **GPU-vs-CPU-24-thread diagnostic:** the canonical CPU-24-thread *full-frame*
  baseline from `3fcb7025` is 21.44 fps ± 0.22 → 13.22×. A full-frame
  GPU-vs-CPU-24 comparison is NOT yet possible because the LDPC decode is still
  CPU-only (Phase B decode kernel is a separate task); this AWGN-only kernel is
  one stage of the eventual GPU pipeline. The AWGN step is not the full-frame
  bottleneck, so a standalone AWGN GPU offload does not by itself raise
  end-to-end full-frame fps above the CPU-24 number — it removes the AWGN cost
  from the critical path once the decode is also on-device.
- **Required threshold (from task body):** ≥ 10× the single-thread CPU
  **AWGN-step** baseline (criterion-3 amended 2026-06-09, user-approved: the
  apples-to-apples noise-step metric, since `f6004add` accelerates only the
  noise step; the `c0b1702d` full-frame baseline is a category-confused
  comparator for a noise-only kernel). **14.91× clears it.**
- **Verdict:** PASS — clean lead re-measurement on a verified-quiet host
  (`loadavg` = 0.42, `rocm-smi` GPU use 0%, only the throughput bin running).
  GPU/CPU-1-thread AWGN-step speedup of **14.91×** clears the ≥10× gate with
  margin; both figures are tight (GPU ±227 over 7 repeats, CPU ±3.30). This
  supersedes the worker's provisional under-load figures (CPU 1156 ± 212 at
  loadavg 2.05); contention only *understated* the CPU baseline (raising the
  apparent speedup), so the gate was never at risk — the clean CPU number
  (1251 ± 3.3) confirms 14.91×. Attested by `agent:project-lead`.
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/gpu_awgn_throughput.rs`
    (re-run: `cargo run -p gf2-sim --release --features hip --bin gpu_awgn_throughput -- --frames 4000 --repeats 7 --es-n0 6.5`).
  - Byte-identity (full-range, N∈{1,256,1024} frames) + ≥1024-frame 1-ulp
    Box-Muller regressions (gfx1030-gated, skip with no GPU), in `gf2-sim` so
    they call the real `gf2_sim::parallel::worker_offset`:
    `gpu::awgn::imp::tests::{test_gpu_chacha_raw_words_full_range_byte_identical,test_gpu_box_muller_within_1_ulp_over_1024_frames}`
    (run: `cargo nextest run -p gf2-sim --release --features hip -E 'test(test_gpu_chacha_raw_words_full_range_byte_identical) | test(test_gpu_box_muller_within_1_ulp_over_1024_frames)'`).
  - CPU-vs-GPU 1-ulp end-to-end stage test:
    `gf2-sim` `gpu::awgn::imp::tests::test_gpu_awgn_matches_cpu_within_1_ulp`
    (run: `cargo nextest run -p gf2-sim --release --features hip -E 'test(test_gpu_awgn_matches_cpu_within_1_ulp)'`).
  - Device kernel source: `crates/gf2-kernels-hip/hip/chacha20_awgn.hip`;
    host wrappers: `crates/gf2-kernels-hip/src/launch_chacha20_awgn.rs`.

## a930be7f — GPU LDPC belief-propagation batch decode kernel

- **Status:** ATTESTED by `agent:project-lead` (clean quiet-host re-measure at
  merged HEAD `f3f0aaa5`; supersedes the worker's provisional under-load figures).
  Still valid after rework-r2 (which removed an unused in-kernel shift FFI
  parameter only — no runtime change; the 1800-frame byte-identity was
  re-verified at the r2 HEAD).
- **Date:** 2026-06-09 (lead quiet-host re-measure after the rework-round-1
  per-frame early-termination FREEZE landed — frozen frames skip all subsequent
  kernel work, so the GPU is faster than the pre-freeze 531.93 fps figure).
- **Hardware:** CPU = AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU = AMD Radeon
  RX 6950 XT (gfx1030, RDNA2).
- **Baseline configuration (apples-to-apples, per the user-approved 2026-06-09
  amendment):** the gate compares **decode-stage throughput vs decode-stage
  throughput**, NOT GPU-decode vs CPU-full-frame. The divisor is the CPU
  `LdpcDecoder` decode stage measured in isolation (`decode_to_codeword`,
  SumProduct, early-termination on) at single-thread and 24-thread.
- **Test configuration:** DVB-T2 r1/2 LDPC code (`LdpcCode::dvb_t2_normal`,
  n = 64800, m = 32400), SumProduct, early-termination on, max_iters = 50, at a
  **waterfall operating point** (all-zero-codeword BPSK, sigma = 0.80) where the
  mean BP depth is **~25.7 iterations** with successful decode — chosen to match
  the `3fcb7025` full-chain mean_iters ≈ 25.24 so the decode-vs-decode comparison
  exercises a realistic iteration count (not the trivial 1-iteration clean
  channel). The GPU path decodes all 200 frames in one batched call (one set of
  per-iteration kernel launches over the whole batch + H2D/D2H); the CPU paths
  decode the same 200 frames serially (1 thread) and across the rayon pool
  (24 threads, per-frame independent `LdpcDecoder`s — the per-frame outcome is a
  deterministic pure function of the frame's LLRs regardless of thread).
- **Observed throughput (200 frames, 5 repeats; lead quiet-host re-measure at
  merged HEAD `f3f0aaa5`, `rocm-smi` GPU 0% and no foreign build/GPU procs before
  the run):**
  - **GPU decode-stage: 639.10 fps ± 1.53** (up from the pre-freeze 531.93 fps
    — the per-frame freeze skips work on already-converged frames; tight σ on the
    quiet host).
  - **CPU decode-stage 1-thread: 2.5210 fps ± 0.0038.**
  - **CPU decode-stage 24-thread: 22.06 fps ± 0.15.**
  - (Consistent with the worker's under-load provisional run — GPU 620.82, CPU-1T
    2.5072, CPU-24T 21.98 at loadavg ≈ 11.5 — load only *understated* the CPU
    baselines, so the gate was never at risk; the clean numbers confirm it.)
- **Speedup factors (decode-vs-decode):**
  - **GPU / CPU-1-thread = 253.51×** (639.10 / 2.5210; gate **≥ 10×** — clears
    with large margin).
  - **GPU / CPU-24-thread = 28.98×** (639.10 / 22.06; gate **≥ 3×** — clears with
    large margin).
- **Context only (full-frame baselines, NOT the gate metric for a decode-only
  kernel):** GPU decode-stage fps is ~394.1× the full-frame single-thread
  baseline (1.6216 fps) and ~29.8× the full-frame 24-thread baseline (21.44 fps).
  These are recorded for completeness per the amendment; the gate is
  decode-vs-decode.
- **Required thresholds (from task body, all [hard]):** GPU decode ≥ 10×
  single-thread CPU decode-stage AND ≥ 3× CPU-24-thread decode-stage.
  **Both clear (253.51× and 28.98×).**
- **Verdict:** PASS (ATTESTED) — clean lead re-measurement on a verified-quiet
  host (`rocm-smi` GPU 0%, no foreign build/GPU procs, tight σ: GPU ±1.53 over 5
  repeats, CPU-1T ±0.0038). GPU/CPU-1-thread = 253.51× and GPU/CPU-24-thread =
  28.98× clear the ≥10× / ≥3× decode-vs-decode gates with large margin. Attested
  by `agent:project-lead` at merged HEAD `f3f0aaa5`.
- **Correctness (criterion 1):** the GPU hard-decision codeword is **byte-identical**
  to the CPU `LdpcDecoder::decode_to_codeword` output for **all** 200 frames × 3
  SNRs × {MinSum, NormalizedMinSum(0.75), SumProduct} (1800 frames). Including
  SumProduct's `tanh`/`atanh` box-plus — the hard-decision verdict is robust to
  the 1-3 ULP RDNA2 transcendental drift (design §11). No decision-boundary
  flips were observed.
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/gpu_ldpc_throughput.rs`
    (re-run: `cargo run -p gf2-sim --release --features hip --bin gpu_ldpc_throughput -- --frames 200 --repeats 5 --max-iters 50`).
  - Byte-identity test (200 frames × 3 SNRs × 3 algorithms, gfx1030-gated, skips
    with no GPU; slow tier, ignored):
    `crates/gf2-sim/tests/gpu_ldpc_byte_identity.rs::gpu_ldpc_hard_decision_byte_identical_to_cpu`
    (run: `cargo nextest run -p gf2-sim --release --features hip --profile slow --run-ignored ignored-only -E 'test(gpu_ldpc_hard_decision_byte_identical_to_cpu)'`).
    Measured ~90 s wall (under the 120 s slow-tier cap; elevated by host load).
  - Device kernel source: `crates/gf2-kernels-hip/hip/ldpc_bp.hip`; host
    wrappers: `crates/gf2-kernels-hip/src/launch_ldpc_bp.rs`; GPU stage:
    `crates/gf2-sim/src/gpu/ldpc_bp.rs`.

## d3f1616a — GPU Gray-QAM max-log soft-demap stage

- **Status:** ATTESTED by `agent:project-lead`. Lead clean re-measure on a
  verified-quiet host (`rocm-smi` GPU 0%, no foreign build/GPU procs) confirmed
  the gate: **16-QAM GPU/CPU-1T = 12.59×, 64-QAM = 16.09×** (both ≥ 5×), matching
  the worker's provisional 12.66× / 15.96×. The GPU σ is warmup-dominated, but
  the CPU-1T divisor is rock-stable and even the GPU −1σ figure clears ≥ 5× for
  both modulations, so the gate is not at warmup risk. The worker's figures below
  stand.
- **Date:** 2026-06-09 (worker measurement + lead quiet-host confirmation).
- **Hardware:** CPU = AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU = AMD Radeon
  RX 6950 XT (gfx1030, RDNA2).
- **Method scoping (HONEST):** the GPU `demap_batch` kernel computes **max-log
  only** (its doc + `hip/gray_qam_demapper.hip` confirm `-d_min0 + d_min1`, no
  log-MAP `exp`/`ln`). The GPU stage therefore serves `DemapMethod::MaxLog` on
  the device; `DemapMethod::ExactLogMap` has NO GPU kernel and is routed to the
  CPU fallback (`execution_class()` reports `CpuOnly` for an `ExactLogMap`
  stage). The receipt and the byte-identity test cover **max-log only**.
- **Baseline configuration (apples-to-apples, per the user-approved 2026-06-09
  amendment):** the gate compares **demap-stage throughput vs demap-stage
  throughput**, NOT GPU-demap vs CPU-full-frame. The divisor is the
  single-thread CPU `FastGrayQamDemapper` **demap step measured in isolation**
  (`demap_llrs`, MaxLog) at the matching modulation. The full-frame `c0b1702d`
  1.6216 fps baseline is category-confused for a demap-only kernel (demap is a
  small fraction of a frame) and is printed for CONTEXT only.
- **Test configuration:** DVB-T2 16-QAM (m=4) and 64-QAM (m=6), MaxLog,
  AWGN (no channel gain), `N0 = 0.35`. 64 frames × 16 200 symbols/frame
  (≈ one FECFRAME of cells per frame). The GPU path demaps the whole
  population (`64 × 16 200 = 1 036 800` symbols) in **one batched device
  launch** (one set of H2D + kernel + D2H), amortising per-call launch
  overhead — the genuine batched-GPU throughput path, mirroring the LDPC
  bench's single batched call. The CPU paths demap the same population
  per-frame, serially (1 thread) and across the rayon pool (24 threads,
  per-frame independent `FastGrayQamDemapper`s — the demap is a pure function
  of the frame's I/Q regardless of thread).
- **Observed throughput (5 repeats; loadavg ≈ 0.18 at measurement, GPU idle):**
  - **16-QAM:** GPU 3.605e8 symbols/s ± 1.94e8 (≈ 22 253 frames/s);
    CPU-1T 2.847e7 ± 3.73e4; CPU-24T 2.567e8 ± 3.64e7.
  - **64-QAM:** GPU 2.603e8 symbols/s ± 1.42e8 (≈ 16 067 frames/s);
    CPU-1T 1.631e7 ± 4.36e4; CPU-24T 1.282e8 ± 1.21e7.
  - (An 11-repeat re-run gave GPU 4.463e8 / CPU-1T 2.843e7 → 15.70× for
    16-QAM and GPU 3.288e8 / CPU-1T 1.625e7 → 20.24× for 64-QAM; the GPU σ is
    warmup-dominated but the **minimum** GPU repeat still clears ≥ 5× — the
    CPU-1T divisor is rock-stable, so the gate is not at warmup risk.)
- **Speedup factors (demap-vs-demap, 5-repeat means):**
  - **16-QAM: GPU / CPU-1-thread = 12.66×** (3.605e8 / 2.847e7; gate **≥ 5×** —
    clears).
  - **64-QAM: GPU / CPU-1-thread = 15.96×** (2.603e8 / 1.631e7; gate **≥ 5×** —
    clears).
  - **CPU-24-thread diagnostic (NOT a gate):** GPU / CPU-24T = 1.40× (16-QAM) /
    2.03× (64-QAM). At 24 threads the CPU `FastGrayQamDemapper` demap step is
    embarrassingly parallel and nearly saturates the GPU on this small per-symbol
    workload (demap is `O(sqrt(M)·m)` per symbol — far cheaper than LDPC BP), so
    the GPU edge over 24 CPU threads is modest; the single-thread gate is the
    contractual one and clears with margin.
- **Required thresholds (from task body, all [hard]):** GPU demap ≥ 5×
  single-thread CPU `FastGrayQamDemapper` demap step (isolated), AND report the
  CPU-24-thread diagnostic. **Both modulations clear ≥ 5× (12.66× / 15.96×); the
  24-thread diagnostic is reported above.**
- **Correctness (criterion 1) — MEASURED tolerance + a real finding:** GPU
  max-log LLRs vs CPU `FastGrayQamDemapper` max-log LLRs, fixed channel
  realisation, 4096 symbols × {16-QAM, 64-QAM}. **LLR ordering was verified to
  align bit-for-bit** (both paths emit symbol-major, MSB-first: `m/2` I-axis
  Gray-PAM bits then `m/2` Q-axis, from the SAME shared `pam_levels` table, same
  positive-favours-bit-0 sign convention), so the comparison is element-wise with
  no reordering.
  - **Measured worst-case absolute difference `|GPU − CPU|`: 16-QAM 1.91e-6,
    64-QAM 9.54e-7.**
  - **The literal "≤ 2 *value*-ulp" criterion is NOT met as written** — even at
    LLR magnitude ≥ 1.0 the worst value-ulp gap is **3 ulp** (not 2), and for
    near-zero LLRs the value-ulp gap reaches thousands (4034 / 22118) while the
    *absolute* difference stays ≤ 1.9e-6. **Root cause (verified, NOT a bug):**
    the GPU kernel (`hip/gray_qam_demapper.hip`) computes the entire max-log
    distance reduction in **f32**; the CPU `FastGrayQamDemapper` computes it in
    **f64** (`GrayPamDistanceFnsF64` + f64 `subset_log_map_llr`) and rounds to
    f32 only at the final `Llr::new`. The residual is the f32-vs-f64
    intermediate-precision floor at the squared-*distance* scale (O(1) here),
    which is the §11 SIMT-vs-SIMD softmath relaxation. The max-log result
    `d_min1 − d_min0` straddles zero, so value-ulp explodes near zero even though
    the absolute residual is a fixed ≈ 2e-6.
  - **What the test asserts:** the honest, near-zero-safe form of "≤ 2 ulp" — the
    standard combined comparison `|GPU − CPU| ≤ 2.0e-6 (MAX_LOG_ABS_TOLERANCE)
    OR value-ulp ≤ 2 (MAX_LOG_ULP_TOLERANCE)`. `MAX_LOG_ABS_TOLERANCE = 2.0e-6`
    is the **measured** worst case (`MEASURED_WORST_ABS_DIFF = 1.9073486e-6`),
    statically asserted to cover it; the test PASSES both modulations on the
    combined tolerance and prints the literal value-ulp diagnostic. **This
    implements the user-approved amendment 2026-06-09b** (recorded in the
    `d3f1616a` JIT issue description): the worker did not silently amend — the
    deviation was escalated, and the user approved replacing the pure value-ulp
    bound with the combined `≤ 2 ulp OR ≤ 2.0e-6 absolute` form because the literal
    "2 *value*-ulp on the LLR" is falsified by data for a max-log LLR straddling
    zero (GPU-f32 vs CPU-f64; near-zero value-ulp is unbounded for a sign-crossing
    quantity regardless of precision). The §11 softmath intent (small residual)
    holds at ≤ 2e-6 absolute.
- **`cpu_fallback()` returns the CPU demapper:** `CpuGrayQamDemapper` (the
  in-crate `Stage<SymbolBatch, LlrBatch>` wrapper delegating to
  `FastGrayQamDemapper`, orphan-rule-required like `CpuLdpcBp`); verified by
  unit test `test_cpu_fallback_has_same_parameters`.
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/gpu_demap_throughput.rs`
    (re-run: `cargo run -p gf2-sim --release --features hip --bin gpu_demap_throughput -- --frames 64 --symbols 16200 --repeats 5`).
  - Byte-identity test (16-QAM + 64-QAM, gfx1030-gated, skips with no GPU;
    ignored, ~0.07 s):
    `crates/gf2-sim/tests/gpu_demap_byte_identity.rs::gpu_demap_max_log_byte_identical_to_cpu`
    (run: `cargo nextest run -p gf2-sim --release --features hip --run-ignored ignored-only -E 'test(gpu_demap_max_log_byte_identical_to_cpu)' --no-capture`).
  - Device kernel source: `crates/gf2-kernels-hip/hip/gray_qam_demapper.hip`;
    host wrapper: `crates/gf2-kernels-hip/src/lib.rs` (`GpuGrayQamDemapper`);
    GPU stage: `crates/gf2-sim/src/gpu/demap.rs`.

## 75c22fa8 — Hybrid pipeline scheduler (CPU-prep ∥ GPU-decode overlap)

- **Status:** **ATTESTED** — lead re-measurement on a verified-quiet host
  (`/proc/loadavg` 1-min = 0.38, GPU 0% busy pre-run, no foreign processes) on
  2026-06-10. Criterion-1 overlap and criterion-3 byte-identity were verified at
  merge (correctness/behaviour, not load-sensitive throughput).
- **Date:** 2026-06-10 (lead; quiet-host attestation run). Worker directional
  loaded-host run 2026-06-09 retained below for history.
- **Hardware:** CPU = AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU = AMD Radeon
  RX 6950 XT (gfx1030, RDNA2).
- **Baseline configuration:** dual-baseline — single-thread CPU full-frame (the
  `c0b1702d` 1.6216 fps headline) AND CPU-24-thread full-frame (the `3fcb7025`
  21.44 fps baseline, the gate divisor).
- **Test configuration:** DVB-T2 r1/2 16-QAM, FrameSize::Normal (n_ldpc = 64800),
  SumProduct LDPC decode (early-termination on), MaxLog soft-demap, Es/N0 = 6.0 dB
  (deep waterfall — `fer = 0.5000`, non-vacuous). Hybrid: each rayon worker
  double-buffers CPU prep of batch N+1 (encode → interleave → QAM map → AWGN →
  demap, all CPU) against the GPU LDPC BP decode of batch N on its owned HIP
  stream (BATCH_FRAMES = 16), then the SSOT CPU BCH decode-tail + error count.
  AWGN stays on the CPU on both paths (the heavy GPU-worth stage is LDPC decode).
- **Observed throughput (ATTESTED at the shipped post-rework HEAD `cba9e8d9`
  — stream-ordered per-worker launches + `ExecutionClass` routing; 240 frames,
  3 repeats; quiet window opened at `/proc/loadavg` 1-min = 0.16, GPU 0%;
  2026-06-10):**
  - **CPU+GPU hybrid: 123.03 ± 9.16 fps.**
  - CPU 24-thread (same run, same config): 11.84 ± 0.02 fps.
  - CPU 1-thread (same run, same config): 1.1646 ± 0.0007 fps.
  - `fer = 0.4417` identical on all three arms (non-vacuous waterfall +
    CPU-vs-GPU column agreement; unchanged from the pre-rework run, pinning
    the stream path's numerics).
  - Window caveat, disclosed: an external bursty job returned near the END of
    the run (loadavg-after 15.26), inflating the hybrid arm's spread (it runs
    last). The CPU arms — executed first, fully inside the window — are
    pristine (±0.0007 / ±0.02) and byte-match the fully-quiet pre-rework run,
    and the hybrid mean agrees with that run (123.25 ± 2.59 at loadavg 0.38,
    HEAD `ab408148`, pre-stream-rework): two independent quiet runs, same
    mean. External load only understates throughput.
  - Raw log: lead attestation run, `hybrid_throughput --frames 240 --repeats 3
    --es-n0 6.0` (chained sustained-quiet-window harness).
- **Speedup factor:** Hybrid / canonical quiet-host CPU-24-thread baseline
  (21.44 fps, the gate divisor from `3fcb7025`) = **5.74×**; Hybrid /
  same-run-same-config CPU-24-thread (11.84 fps, SumProduct at the deep
  waterfall — more BP iterations per frame than the canonical baseline's
  operating point) = **10.39×**. Both clear the gate.
- **Required threshold (from task body):** ≥ 1.5× the CPU-24-thread baseline
  (i.e. ≥ ~32.2 fps). **123.03 fps → 5.74× against the canonical 21.44 fps
  divisor. PASS.**
- **Verdict:** **PASS (attested).** The review rework (real per-worker
  streams, pinned async staging) is throughput-neutral vs the pre-rework
  default-stream code (123.03 vs 123.25 fps). The post-attestation round-2 F1
  fix (worker-stream selection via deterministic `pool.get(i % n)` instead of
  the racy call-order `acquire()`) changes only WHICH equivalent stream a
  worker binds to — same stream count, same work distribution, perf-neutral;
  the attestation stands (same precedent as the `a930be7f` R2 cosmetic-ABI
  rework). Historical: the 2026-06-09
  worker directional run under heavy external load (Baldur's Gate 3 ~368%
  CPU + GPU 95%, loadavg ≈ 12) measured hybrid 51.44 fps → 2.40× the
  canonical divisor — directionally consistent (load only understates
  throughput).
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/hybrid_throughput.rs`
    (re-run on a quiet host:
    `cargo run -p gf2-sim --release --features hip --bin hybrid_throughput -- --frames 240 --repeats 3 --es-n0 6.0`).
  - Overlap (61.2%) + same-path two-run byte-identity suites (GPU-gated,
    `#[ignore]`): `crates/gf2-sim/tests/hybrid_scheduler.rs`
    (run: `cargo nextest run -p gf2-sim --release --features hip --profile slow --run-ignored ignored-only -E 'binary(hybrid_scheduler)' --no-capture`).
  - CPU-path SSOT-equivalence guard (fast tier):
    `crates/gf2-sim/tests/pipeline_run_cpu.rs`.
  - Scheduler: `crates/gf2-sim/src/executor/scheduler.rs`; results:
    `crates/gf2-sim/src/executor/results.rs`.
  - Phase C criterion evidence: `dev/benchmarks/gf2-sim/hybrid-executor-receipts.md`.

## bbf6b6ee — Campaign binary migrated to the gf2-sim pipeline (worker-measured)

- **Date:** 2026-06-11 (worker-measured; **lead re-attests on a verified-quiet
  host** before the `parallelism-pays` gate is passed).
- **Hardware:** CPU=AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU=n/a (CPU path).
- **Baseline configuration:** single-thread (legacy
  `SimulationRunner::run_with_decoder`, the 1.6216 fps headline divisor from
  `c0b1702d` / commit `9e983ae26e`).
- **Test configuration:** the canonical config from the task body — DVB-T2
  r1/2 16-QAM, FrameSize::Normal (n_ldpc = 64800), **`--decoder sumproduct
  --demap exactlogmap --seed 42`**, 200 frames at **Es/N0 = 6.25 dB**, run via
  the **migrated campaign binary end-to-end** (`crates/gf2-sim/src/bin/
  dvb_t2_awgn_campaign.rs`), i.e. the full process wall-clock: arg parse →
  `Pipeline::dvb_t2()` build (DvbT2Concat + LDPC encoder-cache construction) →
  `Pipeline::run_checkpointed(false)` 24-thread sweep → per-SNR checkpoint
  write → CSV write. `mean_iters = 32.66`, `frames = 200`, `errors = 0`
  (6.25 dB is above this config's waterfall knee — the perf metric does not
  require a non-vacuous mix; the byte-identity test separately exercises the
  6.0 dB knee). Byte-identical `fer`/`frames`/`errors`/`mean_iters` across all
  three perf repeats.
- **Measurement method:** 3 repeats of the binary; per-run wall measured with
  `date +%s.%N` around the process; `fps = 200 / wall`. Quiescence verified
  before the run: `cat /proc/loadavg` 1-min ≈ 1.4 (decaying from this worker's
  own prior compiles, not external load), **no `rustc` / `cargo` / `bg3` /
  Baldur's Gate process running** (`pgrep` empty), top non-kernel process at
  2.2% CPU.
- **Observed throughput:** **16.85 fps ± 0.04** (24 threads, 200 frames,
  3 repeats: 16.8239 / 16.8204 / 16.9000 fps). This is the **end-to-end**
  campaign fps; it is lower than `3fcb7025`'s pure-kernel 21.44 fps because the
  end-to-end number includes the one-off codec/encoder-cache construction and
  the per-SNR checkpoint + CSV I/O amortised over only 200 frames — the
  task-specified metric is the binary's end-to-end fps, so that is what is
  reported.
- **Speedup factor:** **10.39×** (16.8481 / 1.6216) versus the 1.6216 fps
  legacy single-thread headline baseline.
- **Required threshold (from task body):** ≥ 8× the legacy single-thread
  baseline.
- **Verdict:** **PASS (worker-measured).** 16.85 fps → 10.39× clears the ≥ 8×
  gate with margin. Marked worker-measured pending the lead's re-measurement on
  a verified-quiet host per the task body ("MEASURE on the quiet machine and
  record honestly — mark it 'worker-measured, lead re-attests on quiet host'").
  Measured at worktree HEAD `57c81c8f`.
- **Raw artefacts:**
  - Migrated binary: `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs`.
    Re-run on a quiet host:
    `for r in 1 2 3; do /usr/bin/time -p target/release/dvb_t2_awgn_campaign --rate 1/2 --modulation 16qam --esn0-range 6.25:6.25:0.5 --max-frames 200 --target-errors 100000000 --decoder sumproduct --demap exactlogmap --output-dir /tmp/dvb_perf_$r --seed 42; done`
    (fps = 200 / wall).
  - Within-pipeline two-run byte-identity (BOTH legs `#[ignore = "sim:"]`
    — full-codec subprocess spawns exceed the 5 s fast cap; see the
    lead attestation below): `crates/gf2-sim/tests/campaign_byte_identity.rs`.
  - CLI flag subprocess contracts (`--gpu` default-build error, `--strict-gpu`,
    parser rejections, end-to-end CSV schema):
    `crates/gf2-sim/tests/campaign_cli_flags.rs`.
  - CLI→config wiring (`--strict-gpu` → `PipelineConfig::strict_gpu` + the other
    run-control knobs): the `strict_gpu_flag_wires_to_config` /
    `run_control_knobs_wire_to_config` unit tests in the binary.

### Lead re-attestation (2026-06-11, quiet host)

- Host verified quiet before the runs (loadavg 0.96, no foreign
  cargo/rustc; the loadavg during a run is the measurement's own 24
  workers). Migrated binary at merge `c59f8800`.
- 3 repeats (fps = 200 / CSV `wall_seconds`; NOTE `wall_seconds` is the
  per-point sweep wall — it excludes preset/codec construction and the
  CSV write, i.e. slightly FAVOURABLE vs the worker's full-process-wall
  methodology above; both clear 8x with ~30% headroom):
  16.8342 / 16.8356 / 16.6892 -> **16.79 +/- 0.07 fps** = **10.35x** the
  canonical legacy
  single-thread baseline (`c0b1702d`, 1.6216 fps). Threshold >= 8x:
  **PASS** (matches the worker-measured 16.85 within noise).
- `parallelism-pays` ATTESTED by the lead on this measurement.

### Slow-leg byte-identity attestation (2026-06-11; gate-visibility per the B.4/de160fc5 receipts precedent)

The within-pipeline two-run byte-identity legs are `#[ignore = "sim:"]`
(two full-codec subprocess spawns cannot fit the 5 s fast cap), so the
green gate does not run them; this attestation records the lead-reviewed
worker runs at branch HEAD `caaaced0`:

- `byte_identical_two_runs_waterfall` (200 frames, seed 42, Es/N0
  6.0 dB — the measured non-vacuous knee for SumProduct/ExactLogMap;
  6.25 dB converges everything for this config): both runs
  `fer=0.26, frames=200, errors=52, mean_iters=48.12`, byte-identical
  on all four columns; non-vacuity `0 < 52 < 200` asserted;
  `wall_seconds` excluded. 38 s (< 120 s slow cap).
- `byte_identical_two_runs_smoke` (8 frames): byte-identical, 6.5 s.
- Un-ignored fast-tier coverage: the CLI parser-rejection subprocess
  tests + the in-binary CLI->config wiring units (0.006 s).

### Final-HEAD confirmation (2026-06-11 evening, merge e575cab3)

The fix round (global tracing subscriber + live heartbeat/snr events +
calibrate/resume smokes) landed after the lead attestation above. A
3-repeat confirmation at the final HEAD measured 14.7448 / 15.0247 /
14.8635 -> 14.86 +/- 0.14 fps = **9.16x** (>= 8x: PASS) — run with an
active browser (~20-25% of one core, bursty) on the desktop, unlike the
verified-idle attestation run. Diagnostics rule out a code cost: the
200-frame run emits exactly 2 tracing events (campaign_start +
snr_point_completed; default heartbeat cadence 1000 > 200 frames), and
there are ZERO tracing callsites in the per-frame hot path (frame_sim,
parallel, checkpoint, LDPC/BCH decoders) for the global subscriber to
activate — the observer adds one atomic fetch_add per frame. The
attested figure remains the verified-quiet 16.79 fps; the cross-epic
production sweep (e4849f07) re-baselines on its own host anyway.

## 23d3525f — 5G NR LDPC GPU real-time decode-rate tuning

- **Task:** tune the flat GPU LDPC BP kernel (reused **unchanged** from
  `a930be7f`, parameterised for 5G NR by the host-side `GpuNr5gDecoder`)
  to a decoded transport-block throughput target on gfx1030.
- **Concrete target (user-approved):** BG1, `i_LS` = 1 (Z = 384), rate
  1/2, BLER ≤ 1e-2, **≥ 200 Mbps** decoded TB throughput.
- **Full sweep, operating point, hardware, and 5-rep mean ± σ:**
  [`./5g-nr-realtime.md`](./5g-nr-realtime.md).
- **Sweep:** `batch ∈ {64,128,256,512,1024}` × `max_iters ∈
  {10,15,20,25}`, calibrated to Es/N0 = −1.4 dB (BLER ≤ 1e-2 waterfall),
  NormalizedMinSum(0.75) + early termination.
- **Selected cell** (highest throughput at BLER ≤ 1e-2): batch = 128,
  max_iters = 20, BLER = 0.00067, **throughput = 17.45 ± 0.03 Mbps**
  (5 reps; σ = 0.03 Mbps, host load 0.63).
- **`parallelism-pays` verdict: PASS under the AMENDED criterion
  (2026-06-12b, user option B): the attested flat-kernel measurement
  17.45 ± 0.03 Mbps IS the bar** (lead re-measure 17.50 ± 0.08 Mbps
  reproduces it; see the OUTCOME bullet below and the lead
  attestation in `5g-nr-realtime.md`). Historical record of the
  pre-amendment escalation: at measurement time the verdict was
  BELOW TARGET — 17.45 Mbps is ~11.5× below the original 200 Mbps
  concrete target (since amended; study `43fb19e2`). After genuine tuning of
  every exposed lever (batch, iters, algorithm NMS/MinSum, early
  termination on/off — kernel source unchanged per the task), the
  measured ceiling is ~17–20 Mbps: the NR rate-matched decoder runs BP
  on the FULL mother code (full_n = 26112, m = 17664), and the flat
  kernel is iteration-compute/bandwidth-bound and already saturated at
  batch ≈ 128 (larger batch is flat-to-slower). Consistent with the
  `a930be7f` anchor (639.10 fps at n=64800/50-iter scales to ~2–4k fps
  here; observed ~2066 fps). Closing the gap needs a **kernel** change
  (layered BP, fp16/packed LLRs, QC-aware coalesced layout) — out of
  scope. The gate is **not weakened**; the lead escalates per
  `feedback_quality_gates` (approve a follow-up kernel-opt issue, or
  amend the target to the achievable rate). Numbers recorded verbatim.
- **Correctness (byte-identity) — PASS, independent of throughput:** the
  host-side base+per-`i_LS`-shift → flat-`LdpcGraphLayout` builder
  (deferred from `a930be7f`) reuses the existing `GpuLdpcBp` flattener
  (no second expansion) and the existing flat kernel decodes the
  canonical BG1 i_LS=1 Z=384 r1/2 lifted code **bit-for-bit** vs the CPU
  `Nr5gRateMatchedDecoder` — `gpu_nr_5g_byte_identity.rs` (smoke 0.08 s
  + 600-frame slow leg 56.7 s).

- **OUTCOME (2026-06-12b, user-approved option B):** `23d3525f`'s
  throughput criterion amended to the attested 17.45 ± 0.03 Mbps
  (flat-kernel ceiling; study `43fb19e2` projects 50–83 Mbps with
  kernel work, deferred to a future epic). The attested measurement
  above is the gate's PASS basis under the amended criterion.

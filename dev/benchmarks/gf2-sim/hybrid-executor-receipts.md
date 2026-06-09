# Phase C — Hybrid CPU/GPU executor: receipts

Phase C of the `gf2-sim` CPU+GPU FEC simulation pipeline epic (`f9717e7e`).
Design contract SSOT: `dev/active/ec530af9-pipeline-design.md` (§3 seek scheme,
§6 multi-arch HIP / overlap, §8 host dispatcher / error mapping, §11 determinism
contract). The canonical per-task throughput entry lives in
[`./parallelism-receipts.md`](./parallelism-receipts.md); this file records the
Phase C success-criterion evidence (overlap attestation + same-path
byte-identity) and cross-references the canonical throughput entry.

## 75c22fa8 — Hybrid pipeline scheduler (CPU-prep ∥ GPU-decode overlap)

The foundational Phase C task: the [`Scheduler`](../../../crates/gf2-sim/src/executor/scheduler.rs)
pairs each rayon worker with one HIP stream (worker `i` owns stream
`i % n_streams`) and double-buffers CPU preparation of batch `N+1` against the
GPU LDPC belief-propagation decode of batch `N`. The runnable public surface
(`Pipeline::run` / `Pipeline::run_with_decoder` / `Pipeline::run_parallel` →
`SimulationResults`, `Builder::with_gpu`, `PipelineConfig::gpu_enabled`) lands
with it so Phase D consumes a real API.

### Criterion 1 — CPU↔GPU overlap > 50%

The scheduler records an [`OverlapTimeline`] of `CpuPrep` / `GpuDecode`
intervals (the same boundaries the `tracing` `gpu_ldpc_decode` spans mark, each
carrying `worker_idx, snr_idx, batch_id, stream_id, stage_name`). The smoke test
`crates/gf2-sim/tests/hybrid_scheduler.rs::hybrid_gpu_cpu_overlap_exceeds_50pct`
runs the hybrid path (8 workers, 128 frames, r1/2 16-QAM, Es/N0 = 6.0 dB
waterfall) and asserts `OverlapTimeline::gpu_overlap_fraction() > 0.5` — the
fraction of GPU-decode wall-time over which some CPU-prep is simultaneously
active (no serial-only gaps).

- **Measured overlap: 61.2%** (24 intervals), on 2026-06-09. **PASS (> 50%).**
- The double buffering runs each worker's CPU prep of batch `N+1` on a
  `std::thread::scope` helper while the worker thread blocks on the GPU LDPC
  decode of batch `N` (the device decoder owns `!Sync` device buffers, so it
  stays on the worker thread; the prep helper captures only `Sync` state).

### Criterion 3 — same-path two-run byte-identity

`crates/gf2-sim/tests/hybrid_scheduler.rs::hybrid_two_run_byte_identical` runs
the SAME hybrid path twice at a fixed seed (4 workers, 64 frames) and asserts
byte-identical `fer` / `frames` / `errors` / `mean_iters`. Because this is the
same device path twice, `mean_iters` IS deterministic run-to-run (the §11
CPU-vs-GPU `mean_iters` exclusion does not apply to a same-path comparison).
**PASS** on 2026-06-09. The determinism rests on the §3 per-frame seek keyed on
the **global** frame index (`worker_offset(seed, snr_idx, 0, g)`), unchanged
from `3fcb7025`; the scheduler's CPU path is a thin wrapper over the SSOT
`run_snr_point`, pinned by `tests/pipeline_run_cpu.rs::pipeline_run_cpu_matches_run_snr_point_ssot`
(byte-for-byte `fer`/`mean_iters` equality vs a direct `run_snr_point`), so
`3fcb7025`'s cross-worker-count {1,2,4,8,24} byte-identity is preserved.

### Criterion 2 — combined throughput ≥ 1.5× the CPU-24-thread baseline

See the canonical §5 receipt entry in
[`./parallelism-receipts.md`](./parallelism-receipts.md) (`75c22fa8`). Summary:

- **Gate:** combined CPU+GPU ≥ 1.5× the CPU-24-thread baseline (21.44 fps from
  `3fcb7025`) on DVB-T2 r1/2 16-QAM at deep waterfall — i.e. ≥ ~32.2 fps.
- **WORKER DIRECTIONAL MEASUREMENT (NOT a quiet-host attestation):** on a host
  with heavy external load (Baldur's Gate 3 at ~368% CPU + GPU 95% busy,
  `/proc/loadavg` ≈ 12), `hybrid_throughput --frames 48 --repeats 1 --es-n0 6.0`
  measured **CPU+GPU hybrid = 51.44 fps** vs CPU-24-thread (under the same load)
  7.22 fps → **7.13×**, and vs the canonical quiet-host 24-thread baseline
  (21.44 fps) → **2.40×**. Both clear the ≥ 1.5× gate with margin even under
  load (external CPU contention only *understates* throughput, and the hybrid
  path is robust because the heavy LDPC decode is off the contended CPU).
  `fer = 0.5000` across all three arms confirms a genuine waterfall (non-vacuous
  decode-success/failure mix) and CPU-vs-GPU agreement on the `fer`/`frames`/
  `errors` columns.
- **REQUIRES LEAD RE-MEASUREMENT ON A VERIFIED-QUIET HOST** before attestation
  (`cat /proc/loadavg` ≈ 0, no `bg3`/foreign cargo/rustc, `rocm-smi --showuse`
  GPU idle), per the `parallelism-pays` gate rules in CLAUDE.md. The directional
  number stands well clear of the gate, so the gate is not at risk, but the
  attested figure must come from a quiet host.
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/hybrid_throughput.rs`
    (re-run on a quiet host:
    `cargo run -p gf2-sim --release --features hip --bin hybrid_throughput -- --frames 240 --repeats 3 --es-n0 6.0`).
  - Overlap + two-run byte-identity suites (GPU-gated, `#[ignore]`):
    `crates/gf2-sim/tests/hybrid_scheduler.rs`
    (run: `cargo nextest run -p gf2-sim --release --features hip --profile slow --run-ignored ignored-only -E 'binary(hybrid_scheduler)' --no-capture`).
  - CPU-path SSOT-equivalence guard (fast tier):
    `crates/gf2-sim/tests/pipeline_run_cpu.rs`.

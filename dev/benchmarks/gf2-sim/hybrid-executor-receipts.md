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
intervals (the same boundaries the `tracing` `pipeline_stage` spans mark, each
carrying `worker_idx, snr_idx, batch_id, stream_id, stage_name, wall_us`). The
smoke test
`crates/gf2-sim/tests/hybrid_scheduler.rs::hybrid_gpu_cpu_overlap_exceeds_50pct`
runs the hybrid path (8 workers, 384 frames = 3 batches of 16 per worker,
r1/2 16-QAM, Es/N0 = 6.0 dB waterfall) and asserts
`OverlapTimeline::gpu_overlap_fraction() > 0.5` — the fraction of GPU-decode
wall-time over which some CPU-prep is simultaneously active (no serial-only
gaps).

- **Measured overlap: 100.0%** (72 intervals), on 2026-06-10 under the REAL
  per-worker stream semantics (review rework: every kernel launch and
  pinned-staged H2D/D2H transfer is enqueued on the worker's owned HIP stream;
  completion is per-stream `hipStreamSynchronize`, no device-wide sync in the
  steady-state loop). **PASS (> 50%).**
- The double buffering runs each worker's CPU prep of batch `N+1` on a
  `std::thread::scope` helper while the worker thread blocks on the
  stream-ordered GPU LDPC decode of batch `N` (the device decoder + pinned
  staging own `!Sync` buffers, so they stay on the worker thread; the prep
  helper captures only `Sync` state). CPU prep is the throughput bound, so
  GPU-decode wall-time is fully covered by concurrent CPU-prep activity.
- History: the original 2026-06-09 figure (61.2%, 8 workers × 128 frames) was
  measured under default-stream semantics with a config of exactly ONE batch
  per worker — i.e. no intra-worker double-buffering at all; it passed only
  because device-wide synchronization stretched GPU intervals across other
  workers' prep. The reworked config (3 batches/worker) is the first to
  exercise the genuine prep(N+1) ∥ decode(N) steady state.

### Criterion 3 — same-path two-run byte-identity

`crates/gf2-sim/tests/hybrid_scheduler.rs::hybrid_two_run_byte_identical` runs
the SAME hybrid path twice at a fixed seed (8 workers, 32 frames per run, one
pipeline build) and asserts byte-identical `fer` / `frames` / `errors` /
`mean_iters`, plus a non-vacuity guard (`0 < errors < frames` at the
waterfall). Because this is the same device path twice, `mean_iters` IS
deterministic run-to-run (the §11 CPU-vs-GPU `mean_iters` exclusion does not
apply to a same-path comparison).

- **NOT `#[ignore]`d** (review rework): the criterion names the literal command
  `cargo test -p gf2-sim --features hip`, so the test executes under it
  (GPU-presence-gated skip on non-GPU hosts). Measured **1.37 s** isolated /
  **2.17 s** under full fast-tier contention via
  `cargo nextest run -p gf2-sim --release --features hip -E 'test(hybrid_two_run_byte_identical)'`
  on the gfx1030 host, 2026-06-10 — inside the 5 s fast-tier cap. **PASS** on
  both back-to-back `cargo test -p gf2-sim --features hip --release`
  invocations (2026-06-10).
- The determinism rests on the §3 per-frame seek keyed on the **global** frame
  index (`worker_offset(seed, snr_idx, 0, g)`), unchanged from `3fcb7025`; the
  scheduler's CPU path is a thin wrapper over the SSOT `run_snr_point`, pinned
  by `tests/pipeline_run_cpu.rs::pipeline_run_cpu_matches_run_snr_point_ssot`
  (byte-for-byte `fer`/`mean_iters` equality vs a direct `run_snr_point`), so
  `3fcb7025`'s cross-worker-count {1,2,4,8,24} byte-identity is preserved.
- The stream-ordered kernel path itself is pinned byte-identical to the
  default-stream path by
  `gf2-kernels-hip::launch_ldpc_bp::tests::test_decode_on_stream_matches_default_stream`
  (fast tier, gfx1030), and the chain-level CPU-vs-GPU three-column contract
  re-verified post-rework via `tests/gpu_ldpc_byte_identity.rs` and
  `tests/gpu_byte_identity.rs` (r1/2 16-QAM config).

### Criterion 2 — combined throughput ≥ 1.5× the CPU-24-thread baseline

See the canonical §5 receipt entry in
[`./parallelism-receipts.md`](./parallelism-receipts.md) (`75c22fa8`). Summary:

- **Gate:** combined CPU+GPU ≥ 1.5× the CPU-24-thread baseline (21.44 fps from
  `3fcb7025`) on DVB-T2 r1/2 16-QAM at deep waterfall — i.e. ≥ ~32.2 fps.
- **ATTESTED (lead quiet-host re-measurement at the shipped post-rework HEAD
  `cba9e8d9`, 2026-06-10):** in a sustained-quiet window (`/proc/loadavg`
  1-min = 0.16 at open, GPU 0%), `hybrid_throughput --frames 240 --repeats 3
  --es-n0 6.0` measured **CPU+GPU hybrid = 123.03 ± 9.16 fps** → **5.74×** the
  canonical 21.44 fps CPU-24-thread divisor (and **10.39×** the
  same-run-same-config CPU-24 arm, 11.84 ± 0.02 fps). **PASS (≥ 1.5×).**
  `fer = 0.4417` identical across all three arms confirms a genuine waterfall
  (non-vacuous decode-success/failure mix) and CPU-vs-GPU agreement on the
  `fer`/`frames`/`errors` columns. An external bursty job returned near the
  run's end (disclosed in the canonical entry), inflating only the
  last-executed hybrid arm's spread; the mean agrees with the fully-quiet
  pre-rework run (123.25 ± 2.59 at `ab408148`) — the stream rework is
  throughput-neutral.
- Historical: the 2026-06-09 worker directional run under heavy external load
  (Baldur's Gate 3 ~368% CPU + GPU 95%, loadavg ≈ 12, 48 frames × 1 repeat)
  measured hybrid 51.44 fps → 2.40× the canonical divisor — directionally
  consistent (external CPU contention only *understates* throughput).
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/hybrid_throughput.rs`
    (re-run on a quiet host:
    `cargo run -p gf2-sim --release --features hip --bin hybrid_throughput -- --frames 240 --repeats 3 --es-n0 6.0`).
  - Overlap + two-run byte-identity suites (GPU-gated):
    `crates/gf2-sim/tests/hybrid_scheduler.rs` — the overlap smoke is slow-tier
    `#[ignore]` (run: `cargo nextest run -p gf2-sim --release --features hip --profile slow --run-ignored ignored-only -E 'test(hybrid_gpu_cpu_overlap_exceeds_50pct)' --no-capture`);
    the two-run byte-identity test is fast-tier (runs under
    `cargo test -p gf2-sim --features hip`, per criterion 3).
  - CPU-path SSOT-equivalence guard (fast tier):
    `crates/gf2-sim/tests/pipeline_run_cpu.rs`.

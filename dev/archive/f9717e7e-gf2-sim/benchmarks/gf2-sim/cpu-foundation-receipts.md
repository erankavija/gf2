# gf2-sim CPU-foundation receipts (story `bcf7776d`)

Story-level rollup for the Phase A **CPU foundation** of the `gf2-sim` epic
(`f9717e7e`). This file records the two story-closure attestations required by
`bcf7776d`'s success criteria — the **per-worker speedup** and the
**byte-identity attestation** — and points to the per-task detail rather than
duplicating it (SSOT: the per-task throughput numbers live in
`parallelism-receipts.md`; the single-thread denominator in
`baseline-single-thread.md`).

## Per-worker speedup (within-SNR frame parallelism, task `3fcb7025`)

- **Single-thread baseline:** 1.6216 fps — DVB-T2 r1/2 16-QAM at Es/N0 = 6.5 dB
  on the legacy `SimulationRunner::run_with_decoder` path (canonical config,
  commit `9e983ae26e`; `baseline-single-thread.md`).
- **24-thread CPU:** 21.44 ± 0.22 fps → **13.22×** speedup (≥ 12× `[hard]`
  threshold for `3fcb7025`), clean re-measure on a verified-quiet host
  (loadavg ≈ 0) at HEAD `ec30b3e1`. Full schema-conformant entry:
  `parallelism-receipts.md` § `3fcb7025`.
- Hardware: AMD Ryzen 9 5900X (12C/24T), CPU-only task.

The 24-thread figure is the canonical CPU baseline that the Phase B GPU receipts
reference (e.g. the GPU LDPC BP `≥ 3×-CPU-24` gate).

## Byte-identity attestation (determinism contract, design-doc §11)

The four columns `fer` / `frames` / `errors` / `mean_iters` are **byte-identical
across worker counts {1, 2, 4, 8, 24}** at a fixed seed, and resume-from-
checkpoint reproduces the uninterrupted final tuple byte-for-byte. `ber` is
excluded (non-associative f32 horizontal reduction; status-quo amendment from
issue `152388f4`). `mean_iters` is a real per-frame BP early-termination depth
(via `DvbT2Concat::decode_soft_counted`), not a sentinel.

This is regression-guarded by two complementary slow-tier suites that share the
four-column / BER-excluded comparison in `crates/gf2-sim/tests/common/mod.rs`:

- `crates/gf2-sim/tests/parallel_determinism.rs` — direct `frame_sim` dispatch
  (task `3fcb7025`).
- `crates/gf2-sim/tests/determinism.rs` — the typestate preset production path
  via `Pipeline::dvb_t2()`, plus heartbeat-resume parity (task `48a0db6c`).

**Attestation:** the full slow-tier sweep
`cargo nextest run -p gf2-sim --release --profile slow --run-ignored ignored-only
-E 'test(determinism)'` passed **12/12 legs** byte-identically on a verified-quiet
host (loadavg ≈ 0.3) at HEAD `2bc48657` on 2026-06-09 — the 6 across-worker
`{1,2}`/`{1,4,8,24}` legs + 3 resume-parity legs from `determinism.rs` and the 3
`parallel_determinism.rs` legs — covering all three named MODCODs (r1/2 16-QAM,
r2/3 64-QAM, r3/4 16-QAM). Each leg ran well under the 120 s/test slow-tier cap
(max 99.7 s).

## Documentation

The determinism contract (CPU-only/parallel byte-identity, the CPU-vs-GPU
`mean_iters` relaxation, and the always-excluded `ber`/`wall_seconds`) is quoted
verbatim from design-doc §11 in `CLAUDE.md`, alongside the `gf2-sim` module map
and the `parallelism-pays` gate note (landed by `48a0db6c`).

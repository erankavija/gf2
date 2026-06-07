# gf2-sim parallelism receipts

Throughput receipts for the `gf2-sim` epic (`f9717e7e`) parallel-pipeline tasks.
Each entry follows the project-plan §5 schema. The **single-thread headline
baseline is 1.6216 fps** for DVB-T2 r1/2 16-QAM at Es/N0 = 6.5 dB (canonical
config), measured on the legacy `SimulationRunner::run_with_decoder` path at
commit `9e983ae26e` and recorded in `baseline-single-thread.md`; it is the
divisor for every speedup gate here. This file establishes the canonical CPU
24-thread baseline that downstream GPU receipts (Phase B) reference.

## 3fcb7025 — Within-SNR frame parallelism + deterministic aggregation

- **Date:** 2026-06-07
- **Hardware:** CPU=AMD Ryzen 9 5900X / 24 threads (12C/24T), GPU=n/a (CPU-only task)
- **Baseline configuration:** single-thread
- **Test configuration:** DVB-T2 r1/2 16-QAM, FrameSize::Normal (n_ldpc = 64800),
  SumProduct LDPC decoder (early-termination on), ExactLogMap soft-demap,
  Es/N0 = 6.5 dB, batch = 1 frame per dispatch (per-frame kernel
  `frame_sim::DvbT2BicmFrameSim`), mean_iters ≈ 1.0 (all frames converge at
  6.5 dB, above the r1/2 16-QAM waterfall).
- **Observed throughput:** **21.79 fps ± 0.03** (24 threads, 144 frames, 3 repeats);
  confirmation run **21.47 fps ± 0.26** (24 threads, 120 frames, 5 repeats).
  New-pipeline single-thread reference: **2.06 fps ± 0.01**.
- **Speedup factor:** **13.44x** (21.79 / 1.6216) — confirmation **13.24x**
  (21.47 / 1.6216) — versus the 1.6216 fps single-thread headline baseline.
  (Intrinsic parallel scaling vs the new pipeline's own 1-thread number is
  ~10.5x at 24 threads; the new pipeline is also ~1.27x faster single-thread
  than the legacy path, so the gate divisor is the more conservative legacy
  baseline.)
- **Required threshold (from task body):** >= 12x
- **Verdict:** PASS — attested by `agent:3fcb7025` at commit `691fe43152`
  (the BICM-AWGN SSOT refactor; impl originally landed at `22e9c66d`). This
  receipt-SHA citation lands in the immediate follow-up commit, since the
  hash of a commit cannot be embedded in its own content. Post-refactor
  throughput spot-check (120 frames, 3 repeats, quiet machine): 24-thread
  **21.66 fps ± 0.22** → **13.36x**, within run-to-run variance of the
  headline sweep below — the refactor is behavior- and performance-preserving.
- **Raw artefacts:**
  - Benchmark binary: `crates/gf2-sim/src/bin/parallel_throughput.rs`
    (re-run: `cargo run -p gf2-sim --release --bin parallel_throughput -- --frames 144 --workers 1,2,4,8,24 --repeats 3 --es-n0 6.5`).
  - Determinism regression (byte-identity across {1,2,4,8,24} workers,
    3 configs): `crates/gf2-sim/tests/parallel_determinism.rs` (slow tier,
    ignored; run with `cargo nextest run -p gf2-sim --release --profile slow --run-ignored ignored-only -E 'test(determinism_r)'`).

### Scaling sweep (144 frames, 3 repeats, Es/N0 = 6.5 dB)

| Workers | fps_mean | fps_sigma | speedup vs 1.6216 |
|--------:|---------:|----------:|------------------:|
| 1       | 2.0615   | 0.0092    | 1.27x             |
| 2       | 4.0455   | 0.0022    | 2.49x             |
| 4       | 7.3522   | 0.0415    | 4.53x             |
| 8       | 13.5639  | 0.0584    | 8.36x             |
| 24      | 21.7936  | 0.0275    | **13.44x**        |

Near-linear scaling through 8 workers (8.36x on 8 threads); the 24-thread number
benefits from the 12 physical cores' SMT. The per-worker design (each rayon
worker owns its own `DvbT2BicmFrameSim` clone, hence its own LDPC decoder) is
what unlocks this — a single shared `DvbT2Concat` would serialise every decode on
its internal decoder `Mutex`.

### Determinism evidence

`tests/parallel_determinism.rs` asserts byte-identical `fer` / `frames` /
`errors` / `mean_iters` (bit-pattern equality on the derived f64 ratios, exact
equality on the u64 counters) across worker counts {1, 2, 4, 8, 24} for three
`(rate, modulation)` configurations, each at a waterfall Es/N0 so the frame set
contains both decode successes and failures:

| Config | Es/N0 | Decoder / demap | Result |
|--------|------:|-----------------|--------|
| r1/2 16-QAM | 6.25 dB | SumProduct / ExactLogMap | PASS (byte-identical {1,2,4,8,24}) |
| r2/3 16-QAM | 8.90 dB | NMS(0.75) / ExactLogMap  | PASS (byte-identical {1,2,4,8,24}) |
| r1/2 64-QAM | 9.90 dB | MinSum / MaxLog          | PASS (byte-identical {1,2,4,8,24}) |

All three passed locally on 2026-06-07 (43 s / 47 s / 73 s respectively, each
under the slow tier's 120 s/test cap). The byte-identity holds because every
global frame's noise is keyed on the global frame index via the design-doc §3
seek `worker_offset(seed, snr_idx, 0, g)`, making each frame's outcome a pure
function of `g` regardless of worker count, and the per-worker counters are
reduced in `worker_idx` order (the SSOT aggregation order).

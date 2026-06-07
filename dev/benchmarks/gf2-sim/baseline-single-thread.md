# Single-thread baseline — `simulation.rs::run_with_decoder`

Canonical single-thread throughput baseline for the **gf2-sim** epic. Every
`parallelism-pays`-gated task measures its speedup claim against the numbers in
this receipt. The baseline is measured on the **legacy**
`crates/gf2-coding/src/simulation.rs::SimulationRunner::run_with_decoder` path —
the system the new pipeline replaces — so parallel-pipeline receipts compare
against the path being migrated, not against the new pipeline on one thread.

- **Commit:** `9e983ae26e776d0e120a3e0fdac0f2f8805ee3fa` (`9e983ae26e`)
- **Date:** 2026-06-07
- **Issue:** c0b1702d

## Hardware / toolchain

| Item | Value |
|------|-------|
| CPU | AMD Ryzen 9 5900X (12 cores / 24 threads) |
| RAM | 31 GiB |
| OS | Linux 7.0.10-arch1-1 x86_64 |
| rustc | 1.95.0 (59807616e 2026-04-14) |
| Build profile | `--release` (workspace `[profile.release]`: `lto = "thin"`, `codegen-units = 1`) |
| Threads | Pinned to **1** via `rayon::ThreadPoolBuilder::new().num_threads(1).build_global()` |

## Methodology

The harness (`dev/benchmarks/gf2-sim/baseline_runner/`) is a standalone,
non-workspace binary crate that depends on `gf2-coding` by relative path. It
drives the exact legacy path the campaign binary uses
(`crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs`):

```text
BBFRAME -> BCH+LDPC encode -> bit interleave -> Gray-QAM map -> AWGN
                                                                  |
BBFRAME <- BCH+LDPC decode <- bit deinterleave <- Gray-QAM soft demap
```

- `BicmFecEncoder` wraps `DvbT2Concat` as a `BlockEncoder`.
- `BicmAwgnChannel` implements `ChannelModel`: bit-interleave -> `ModemSpec`
  Gray-square-QAM map -> Box-Muller AWGN on I and Q -> soft demap (method per
  cell) -> deinterleave LLRs.
- The per-frame decode closure calls `DvbT2Concat::decode_soft` and is passed to
  `SimulationRunner::run_with_decoder`, which runs the sequential sweep.
- One Es/N0 point per `run_with_decoder` invocation; `min_errors = usize::MAX`
  and `max_frames = 200` so every cell runs all 200 frames (a throughput
  baseline, not an accuracy sweep — early-stop on errors is disabled).
- Fixed `rng_seed = 42`. `max_decoder_iterations = 50`.

### Sweep matrix (27 cells)

- **3 MODCODs:** r1/2 16-QAM, r2/3 16-QAM, r1/2 64-QAM.
- **3 Es/N0 points per MODCOD** (pre-waterfall / waterfall-mid / deep-waterfall),
  anchored on the ETSI TS 102 831 Table 44 QEF C/N thresholds (6.0 / 8.9 /
  9.9 dB respectively):
  - r1/2 16-QAM: 5.00 / 6.25 / 7.50 dB
  - r2/3 16-QAM: 7.90 / 8.90 / 10.40 dB
  - r1/2 64-QAM: 8.90 / 9.90 / 11.40 dB
- **3 decoder x demap pairs:** (SumProduct, ExactLogMap),
  (NormalizedMinSum(0.75), ExactLogMap), (MinSum, MaxLog).

`frames_per_sec = frames / wall_seconds`, where `wall_seconds` is wall-clock
time around the single `run_with_decoder` call for that cell (encoder build and
LDPC graph allocation excluded — those happen before the timed call).

> **Note on `mean_iters`.** The per-frame decode closure returns
> `DecoderResult::success(...)` (iterations sentinel = 1) when LDPC converges, so
> `mean_iters` reads 1.0 for converged cells and 50.0 (the BP cap) for cells
> where decoding never converges. It is a coarse convergence indicator, not the
> true internal BP iteration count for converged frames; `frames_per_sec` is the
> load-bearing throughput metric.

## Headline table (frames/sec, 1 thread, 200 frames/cell)

| Rate | Mod   | Es/N0 (dB) | SumProduct+ExactLogMap | NMS(0.75)+ExactLogMap | MinSum+MaxLog |
|------|-------|-----------:|-----------------------:|----------------------:|--------------:|
| 1/2  | 16qam | 5.00       | 1.136                  | 1.636                 | 1.636         |
| 1/2  | 16qam | **6.25**   | **1.622**              | 1.645                 | 1.647         |
| 1/2  | 16qam | 7.50       | 3.313                  | 2.152                 | 3.968         |
| 2/3  | 16qam | 7.90       | 0.877                  | 0.915                 | 0.919         |
| 2/3  | 16qam | 8.90       | 1.108                  | 0.906                 | 0.915         |
| 2/3  | 16qam | 10.40      | 3.609                  | 3.283                 | 3.662         |
| 1/2  | 64qam | 8.90       | 1.108                  | 1.637                 | 1.641         |
| 1/2  | 64qam | 9.90       | 1.111                  | 1.642                 | 1.647         |
| 1/2  | 64qam | 11.40      | 2.305                  | 1.651                 | 2.535         |

Full per-cell data (frames, wall_seconds, ber, fer, mean_iters, commit, date):
`dev/benchmarks/gf2-sim/baseline-single-thread.csv`.

## [hard] sanity check — canonical config

The canonical config is **r1/2 16-QAM, Es/N0 = 6.25 dB, SumProduct,
ExactLogMap**. The prior opportunistic figure was **1.617 fps**.

- **Measured:** **1.6216 fps** (200 frames / 123.337 s; FER = 0.0, BER = 0.0).
- **Drift vs 1.617 fps:** +0.29% — **PASS** (well inside +/-10%).

Cross-check that the harness exercises the right path: the all-failure MinSum
cells at the pre-waterfall SNR (e.g. r1/2 16-QAM @ 5.00 dB, MinSum/MaxLog,
all 200 frames fail at 50 BP iterations) measure ~1.6 fps, consistent with the
project's earlier opportunistic calibration of the same legacy path
(`dev/benchmarks/dvb_t2_awgn/smoke/calibration/`, ~1.0 fps for the equivalent
all-fail MinSum point — same order of magnitude, differences attributable to
SNR and host load). The match confirms the baseline measures
`run_with_decoder`, not a faster substitute path.

## Total wall-time

**3581.0 s (59.7 min)** for all 27 cells on one thread.

## Reproducing & emitting a delta

The harness is reusable: re-invoke the same binary at a later commit and compare
against this committed CSV.

```bash
# Build (from repo root):
cargo build --release \
    --manifest-path dev/benchmarks/gf2-sim/baseline_runner/Cargo.toml

# Re-run the full matrix to a fresh CSV:
./dev/benchmarks/gf2-sim/baseline_runner/target/release/baseline_runner \
    --output /tmp/baseline-rerun.csv

# Re-run AND print a per-cell delta (fps_new vs fps_ref, delta%) against the
# committed baseline:
./dev/benchmarks/gf2-sim/baseline_runner/target/release/baseline_runner \
    --output /tmp/baseline-rerun.csv \
    --compare dev/benchmarks/gf2-sim/baseline-single-thread.csv
```

The CSV is written incrementally (header up front, one row appended per cell as
it finishes), so an interrupted run still leaves a valid partial CSV on disk.

## Files

- `dev/benchmarks/gf2-sim/baseline_runner/` — standalone harness crate
  (`Cargo.toml`, `src/main.rs`, `.gitignore`). Non-workspace member; declares its
  own `[workspace]` table.
- `dev/benchmarks/gf2-sim/baseline-single-thread.csv` — 27-row results CSV.
- `dev/benchmarks/gf2-sim/baseline-single-thread.md` — this receipt.

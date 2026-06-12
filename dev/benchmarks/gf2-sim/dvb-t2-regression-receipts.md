# DVB-T2 byte-identity regression receipts (issue `0d9cb8e3`, Phase D close)

This file contains the 200-frame off-test completion evidence required by the
AMENDMENT 2026-06-12 on issue `0d9cb8e3`. The amendment reduced the in-test
frame count to 50 (to fit the 120 s slow-tier cap) while requiring a one-time
200-frame run per MODCOD as completion evidence for the original 200-frame
intent.

## Host

- CPU: AMD Ryzen 9 5900X, 24 threads
- GPU: AMD Radeon RX 6900 XT (gfx1030)
- RAM: 31 GiB
- OS: Linux 7.0.10-arch1-1
- rustc: 1.95.0

## Mechanism

The `GF2_SIM_REGRESSION_FRAMES` environment variable overrides the default
50-frame slow leg count. The test binary is invoked via `cargo test` (not
nextest, which enforces the 120 s cap):

```bash
GF2_SIM_REGRESSION_FRAMES=200 cargo test -p gf2-sim --all-features --release \
    --test dvb_t2_regression -- test_dvb_t2_regression_50f_<modcod> \
    --ignored --nocapture
```

Seed: `0xDE16_0FC5`, decoder: SumProduct + ExactLogMap.

## Column legend

- A: Mode A (CPU-only, parallelism=1)
- B: Mode B (CPU-parallel, parallelism=24)
- C: Mode C (CPU+GPU on gfx1030, with_gpu=true)
- A==B: all four columns `frames/errors/fer/mean_iters` byte-identical
- A==C: three columns `frames/errors/fer` byte-identical; `mean_iters` logged

## Results (200 frames per MODCOD, seed 0xDE16_0FC5)

### r1/2 16-QAM @6.0 dB

Command:
```
GF2_SIM_REGRESSION_FRAMES=200 cargo test -p gf2-sim --all-features --release \
    --test dvb_t2_regression -- test_dvb_t2_regression_50f_r12_16qam \
    --ignored --nocapture
```

Wall time: 189 s (Mode A alone at ~1.06 fps; B+C add seconds)

| Mode | frames | errors | fer      | mean_iters |
|------|--------|--------|----------|------------|
| A    | 200    | 58     | 0.290000 | 48.330000  |
| B    | 200    | 58     | 0.290000 | 48.330000  |
| C    | 200    | 58     | 0.290000 | 48.330000  |

A==B (four-column, byte-identical). A==C (three-column, byte-identical).
mean_iters C diff vs A: 0.0000.

### r1/2 64-QAM @10.3 dB

Wall time: 202 s

| Mode | frames | errors | fer      | mean_iters |
|------|--------|--------|----------|------------|
| A    | 200    | 72     | 0.360000 | 49.735000  |
| B    | 200    | 72     | 0.360000 | 49.735000  |
| C    | 200    | 72     | 0.360000 | 49.735000  |

A==B (four-column, byte-identical). A==C (three-column, byte-identical).
mean_iters C diff vs A: 0.0000.

### r2/3 16-QAM @8.8 dB

Wall time: 241 s

| Mode | frames | errors | fer      | mean_iters |
|------|--------|--------|----------|------------|
| A    | 200    | 100    | 0.500000 | 47.250000  |
| B    | 200    | 100    | 0.500000 | 47.250000  |
| C    | 200    | 100    | 0.500000 | 47.250000  |

A==B (four-column, byte-identical). A==C (three-column, byte-identical).
mean_iters C diff vs A: 0.0000.

### r2/3 64-QAM @13.8 dB

Wall time: 252 s

| Mode | frames | errors | fer      | mean_iters |
|------|--------|--------|----------|------------|
| A    | 200    | 130    | 0.650000 | 48.705000  |
| B    | 200    | 130    | 0.650000 | 48.705000  |
| C    | 200    | 130    | 0.650000 | 48.705000  |

A==B (four-column, byte-identical). A==C (three-column, byte-identical).
mean_iters C diff vs A: 0.0000.

### r3/4 16-QAM @10.0 dB

Wall time: 332 s

| Mode | frames | errors | fer      | mean_iters |
|------|--------|--------|----------|------------|
| A    | 200    | 124    | 0.620000 | 47.535000  |
| B    | 200    | 124    | 0.620000 | 47.535000  |
| C    | 200    | 124    | 0.620000 | 47.535000  |

A==B (four-column, byte-identical). A==C (three-column, byte-identical).
mean_iters C diff vs A: 0.0000.

### r3/4 64-QAM @15.4 dB

Wall time: 308 s

| Mode | frames | errors | fer      | mean_iters |
|------|--------|--------|----------|------------|
| A    | 200    | 62     | 0.310000 | 44.015000  |
| B    | 200    | 62     | 0.310000 | 44.015000  |
| C    | 200    | 62     | 0.310000 | 44.015000  |

A==B (four-column, byte-identical). A==C (three-column, byte-identical).
mean_iters C diff vs A: 0.0000.

## Summary

All six MODCODs at 200 frames: A=B=C on all three contractual columns
(`frames`, `errors`, `fer`); A=B also on `mean_iters` (four-column CPU
contract). The `mean_iters` CPU-vs-GPU diff is 0.0000 for all six on this
host — logged for diagnostics only; `mean_iters` is EXCLUDED from the
CPU-vs-GPU contract (§11's rationale anticipates per-frame ±1 iteration
drift from RDNA2 transcendental ULPs; none manifested at these seeds).
Non-vacuity holds for all six (errors in range [58, 130] out of 200 frames).

## 50-frame slow-tier observed values (AMENDMENT 2026-06-12 in-test legs)

For reference — the 50-frame legs that run under nextest with the 120 s cap:

| MODCOD             | Es/N0 (dB) | errors/50 | fer      | mean_iters | wall (s) |
|--------------------|------------|-----------|----------|------------|----------|
| r1/2 16-QAM        | 6.0        | 12        | 0.240000 | 48.220000  | 48.5     |
| r1/2 64-QAM        | 10.3       | 21        | 0.420000 | 49.860000  | 52.1     |
| r2/3 16-QAM        | 8.8        | 27        | 0.540000 | 47.180000  | 61.4     |
| r2/3 64-QAM        | 13.8       | 32        | 0.640000 | 48.480000  | 64.0     |
| r3/4 16-QAM        | 10.0       | 31        | 0.620000 | 47.120000  | 84.2     |
| r3/4 64-QAM        | 15.4       | 17        | 0.340000 | 42.480000  | 76.0     |

All legs: A=B=C byte-identical on all contractual columns.

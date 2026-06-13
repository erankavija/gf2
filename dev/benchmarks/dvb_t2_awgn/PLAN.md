# DVB-T2 AWGN campaign — SNR sweep plan (e4849f07)

Plan for the production FER-vs-Es/N0 campaign for the six in-scope
DVB-T2 MODCODs, run on the **`gf2-sim` hybrid CPU+GPU pipeline**.

## Runner

The production campaign uses the migrated `gf2-sim` campaign binary
`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` (issue `bbf6b6ee`),
backed by `Pipeline::dvb_t2` + `Scheduler::run_sweep_checkpointed`.
It supersedes the legacy `SimulationRunner`-based binary. With a
`--features hip` build and `--gpu`, the heavy LDPC belief-propagation
decode and Gray-QAM soft-demap stages are offloaded to the HIP/ROCm
device (the rest of the chain — BCH+LDPC encode, bit interleave, AWGN,
aggregation — runs across the rayon pool concurrently).

Decoder/demap configuration (calibration-recommended, see `README.md`):
`--decoder sumproduct --demap exactlogmap`. These are within the epic's
"tuning parameters / schedule" allowance and close most of the
implementation gap that plain min-sum + max-log leaves open.

## Reference anchors (ETSI TS 102 831 v1.2.1 Table 44)

Required raw (C/N)₀ for BER = 1×10⁻⁷ after LDPC (Normal frame,
AWGN/Gaussian channel). These are the QEF thresholds the simulated
FER=10⁻⁴ waterfall should bracket.

| Rate | Modulation | QEF C/N (dB) | Spec. eff. (b/s/Hz) |
|------|-----------|--------------|---------------------|
| 1/2  | 16-QAM    | 6.0          | 1.99                |
| 2/3  | 16-QAM    | 8.9          | 2.66                |
| 3/4  | 16-QAM    | 10.0         | 2.99                |
| 1/2  | 64-QAM    | 9.9          | 2.98                |
| 2/3  | 64-QAM    | 13.5         | 3.99                |
| 3/4  | 64-QAM    | 15.1         | 4.48                |

## Per-config production SNR sweep

Each sweep brackets the waterfall knee measured by the `gf2-sim`
DVB-T2 byte-identity regression
(`dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md`, SumProduct +
ExactLogMap, 200 frames/MODCOD at the QEF anchor) and extends through
FER = 10⁻⁴. Step 0.05 dB near the waterfall. The epic's [hard] criterion
(as amended 2026-06-13) requires ≥ 10⁶ frames at the deepest plotted SNR
point and ≥ 100 frame errors at the point bracketing FER = 10⁻⁴ from
above (FER ≥ 10⁻⁴); see `CLOSURE.md` and the e4849f07 issue description.

The waterfall is a sharp cliff (~1 decade of FER per 0.045 dB). Windows
use a 0.05 dB step and stop ~0.05 dB past the estimated FER=10⁻⁴
crossing, so points far below 10⁻⁴ do not waste `--max-frames`.
`--max-frames 1200000` ≥ the epic gloss's "≥10⁶ frames at the deepest
plotted SNR" and yields ~120 frame errors at the FER=10⁻⁴ bracketing
point.

| Rate | Mod    | `--esn0-range` (start:stop:step) | `--target-errors` | `--max-frames` |
|------|--------|----------------------------------|-------------------|----------------|
| 1/2  | 16qam  | `5.85:6.20:0.05`                 | 100               | 1200000        |
| 2/3  | 16qam  | `8.70:9.00:0.05`                 | 100               | 1200000        |
| 3/4  | 16qam  | `9.85:10.20:0.05`                | 100               | 1200000        |
| 1/2  | 64qam  | `10.15:10.50:0.05`               | 100               | 1200000        |
| 2/3  | 64qam  | `13.65:14.00:0.05`               | 100               | 1200000        |
| 3/4  | 64qam  | `15.25:15.60:0.05`               | 100               | 1200000        |

Seed: `--seed 42` (recorded in `README.md`).

## Invocation

The driver `run_campaign.sh` runs all six configs serially on the single
GPU (concurrent GPU campaigns contend on the one gfx1030 device) and is
resumable — re-invoking it picks each config up from its last completed
SNR checkpoint:

```bash
bash dev/benchmarks/dvb_t2_awgn/run_campaign.sh
```

Equivalent single-config invocation:

```bash
cargo run -p gf2-sim --release --features hip --bin dvb_t2_awgn_campaign -- \
    --rate 1/2 --modulation 16qam \
    --esn0-range 5.85:6.25:0.05 \
    --target-errors 100 --max-frames 1500000 \
    --decoder sumproduct --demap exactlogmap \
    --gpu --seed 42 \
    --output-dir dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam
```

Each run is resumable: if interrupted (SIGINT / kill), re-invoke the
identical command with `--resume` appended; it continues from the next
unfinished SNR point. The deterministic columns of the final CSV
(es_n0_db, fer, frames, errors, mean_iters) are byte-identical to an
uninterrupted run; `wall_seconds` and `ber` are runtime / SIMD-f32 /
RDNA2-transcendental dependent and excluded from that guarantee.

## Output layout

Each config runs into a scratch subdir `curve_<rate>_<mod>/` (the whole
subdir is gitignored — see `.gitignore`). The committed deliverables are
the **flat** files promoted out of each scratch subdir:

- `curve_<rate>_<mod>.csv` — the campaign CSV.
- `curve_<rate>_<mod>.png` — the simulated-vs-ETSI overlay (from `plot.py`).
- `tracing_<rate>_<mod>.jsonl` — the structured tracing log.

## Plotting (after each run)

```bash
python3 dev/benchmarks/dvb_t2_awgn/plot.py \
    --curve-csv dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam.csv \
    --reference-toml crates/gf2-coding/data/dvb_t2_tr102831_reference.toml \
    --output dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam.png
```

## Throughput / wall-clock (GPU)

Host: AMD Ryzen 9 5900X (24 threads) + AMD Radeon RX 6950 XT (gfx1030,
RDNA2), 31 GiB RAM, Linux 7.0.10, rustc 1.95.0.

The GPU LDPC BP batch decode is ~29× the 24-thread CPU baseline
(`dev/benchmarks/gf2-sim/parallelism-receipts.md`). Shallow (high-FER)
SNR points hit `--target-errors 100` in ~1–2 k frames and finish in
seconds; the deep waterfall points near FER = 10⁻⁴ need ~10⁶ frames each
and dominate the wall-clock. A full six-config campaign is a multi-hour
GPU run; launch via `run_campaign.sh` unattended (checkpoints per SNR
point, resumes safely).

## Closure criteria (epic 2928ccce + task e4849f07)

For each of the 6 curves (success criteria as amended 2026-06-13 — see
`CLOSURE.md` and the e4849f07 issue description):
- [x] CSV produced; deepest plotted SNR point has ≥ 10⁶ frames and the
  above-cliff point bracketing FER = 10⁻⁴ (FER ≥ 10⁻⁴) has ≥ 100 frame
  errors. (Amended from "≥ 100 errors at each bracketing point"; the
  near-vertical N=64800 waterfall puts the sub-10⁻⁴ point at FER ~ 10⁻⁵
  where ≥ 100 errors would need 5–24 × 10⁶ frames.)
- [x] PNG overlay (simulated vs TS 102 831 anchor) produced by `plot.py`.
- [x] Gap to the TS 102 831 QEF C/N anchor at FER = 10⁻⁴: ≤ 0.5 dB for
  the 16-QAM curves, ≤ 0.65 dB for the 64-QAM curves. (Amended from a
  uniform ≤ 0.5 dB; the TS 102 831 anchors assume Genie-Aided demapping,
  optimistic for high-order constellations — see `CLOSURE.md`.) The
  TS 102 831 anchor is a BER=10⁻⁷-after-LDPC QEF threshold; the FER at
  that threshold is ≈ 10⁻⁴ (see the reference TOML header).
- [x] Per-curve closure note recording the achieved gap (`CLOSURE.md`).

All six curves + plots + README + closure note are committed under this
directory; e4849f07 and epic 2928ccce are closeable.

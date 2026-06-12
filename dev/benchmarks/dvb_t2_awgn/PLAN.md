# DVB-T2 AWGN campaign — SNR sweep plan (e4849f07)

Plan for the production FER-vs-Es/N0 campaign for the six in-scope
DVB-T2 MODCODs. The campaign runner binary (`dvb_t2_awgn_campaign`,
issue 152388f4) and the resumable observable `SimulationRunner`
(issue fd73e8a8) are complete; this document is the operator's plan
for the actual multi-day runs.

## Status of this document

- **Calibration smoke**: delivered by the project-lead session
  (see `README.md` in this directory for the calibration results).
- **Production runs**: NOT yet executed. They require multi-day
  wall-clock and are the operator's responsibility (per the
  agreed Wave-5 scope: lead delivers calibration, user runs prod).
  Issue `e4849f07` stays open until the production artefacts land.

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

Each sweep brackets the QEF threshold to capture the FER waterfall
through FER = 10⁻⁴. Step 0.25 dB near the waterfall. The epic's
[hard] criterion requires ≥ 100 frame errors at every SNR point
bracketing FER = 10⁻⁴, i.e. ≥ 10⁶ frames at the deepest plotted SNR.

| Rate | Mod    | `--esn0-range` (start:stop:step) | `--target-errors` | `--max-frames` |
|------|--------|----------------------------------|-------------------|----------------|
| 1/2  | 16qam  | `4.5:6.5:0.25`                   | 100               | 10000000       |
| 2/3  | 16qam  | `7.5:9.5:0.25`                   | 100               | 10000000       |
| 3/4  | 16qam  | `8.5:10.5:0.25`                  | 100               | 10000000       |
| 1/2  | 64qam  | `8.5:10.5:0.25`                  | 100               | 10000000       |
| 2/3  | 64qam  | `12.0:14.0:0.25`                 | 100               | 10000000       |
| 3/4  | 64qam  | `13.5:15.5:0.25`                 | 100               | 10000000       |

Seed: `--seed 42` (any fixed seed; record it in the run README).

## Invocation (one per config)

```bash
BIN=./target/release/dvb_t2_awgn_campaign
$BIN --rate 1/2 --modulation 16qam \
     --esn0-range 4.5:6.5:0.25 \
     --target-errors 100 --max-frames 10000000 \
     --seed 42 \
     --output-dir dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam
# ... repeat for the other five rows of the table above.
```

Each run is resumable: if interrupted (SIGINT / kill), re-invoke the
identical command with `--resume` appended; it continues from the
next unfinished SNR point. The deterministic columns of the final
CSV (es_n0_db, fer, frames, errors, mean_iters) are byte-identical
to an uninterrupted run; `wall_seconds` and `ber` are runtime /
SIMD-f32-dependent and excluded from that guarantee.

## Plotting (after each run)

```bash
python3 dev/benchmarks/dvb_t2_awgn/plot.py \
    dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam/curve_1_2_16qam.csv \
    crates/gf2-coding/data/dvb_t2_tr102831_reference.toml
# produces curve_1_2_16qam.png alongside the CSV.
```

## Expected wall-clock (rough)

Host (calibration host): AMD Ryzen 9 5900X, 24 threads, 31 GiB RAM,
Linux 7.0.3, rustc 1.95.0.

Dominant costs per config:
- LDPC encoder (Richardson-Urbanke) preprocessing: ~3-6 min, once
  per process (cached after first encode).
- Decode: belief-propagation min-sum, AVX2 + Rayon. Throughput is
  the limiting factor; the deepest SNR point needs ≥ 10⁶ frames to
  collect ≥ 100 frame errors at FER ≈ 10⁻⁴.

A single config's full sweep to FER = 10⁻⁴ is expected to take on the
order of hours to a day depending on rate/modulation (deeper SNR =
fewer errors per frame = more frames needed). All six configs is a
multi-day campaign. Launch each unattended; the runner checkpoints
per SNR point and resumes safely.

## Closure criteria (epic 2928ccce + task e4849f07)

For each of the 6 curves:
- [ ] CSV produced with ≥ 100 frame errors at the SNR points
  bracketing FER = 10⁻⁴.
- [ ] PNG overlay (simulated vs TS 102 831 anchor) produced by
  `plot.py`.
- [ ] Gap to the TS 102 831 QEF C/N anchor ≤ 0.5 dB at FER = 10⁻⁴
  (the epic's [hard] criterion). NOTE: the TS 102 831 anchor is a
  BER=10⁻⁷-after-LDPC QEF threshold, not a FER=10⁻⁴ point; when
  comparing, account for the ~small offset between FER=10⁻⁴ and the
  QEF definition (see the reference TOML header). If a curve cannot
  reach ≤ 0.5 dB, escalate per the amendment/escalation policy before
  closing e4849f07.
- [ ] Per-curve closure note recording the achieved gap.

Once all six curves + plots + README + closure note are committed
under this directory, e4849f07 and then epic 2928ccce can be closed.

## Legacy forward-pointer (Phase D close, issue 0d9cb8e3)

This directory (`dev/benchmarks/dvb_t2_awgn/`) contains the pre-gf2-sim
pipeline campaign artefacts. The v2 gf2-sim pipeline has superseded the
legacy `SimulationRunner`-based binary:

- **New campaign binary**: `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs`
  (migrated in issue `bbf6b6ee`; same CLI, backed by `Pipeline::run`).
- **Throughput and parallel-executor receipts**: `dev/benchmarks/gf2-sim/parallelism-receipts.md`
  and `dev/benchmarks/gf2-sim/cpu-foundation-receipts.md`.
- **GPU-stages receipts**: `dev/benchmarks/gf2-sim/gpu-stages-receipts.md`.
- **DVB-T2 byte-identity regression receipts** (200-frame off-test completion
  evidence for all six MODCODs, issue `0d9cb8e3`):
  `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md`.

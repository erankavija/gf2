# DVB-T2 AWGN production campaign — results (e4849f07 / epic 2928ccce)

Production FER-vs-Es/N₀ curves for the six in-scope DVB-T2 Normal-frame
MODCODs (rates 1/2, 2/3, 3/4 × 16-QAM, 64-QAM), run on the **`gf2-sim`
hybrid CPU+GPU pipeline** with SumProduct LDPC decoding + ExactLogMap
soft-demapping, compared against the ETSI TS 102 831 Table 44 QEF
anchors.

See `CLOSURE.md` for the per-curve gap analysis and the criterion
amendment record. See `PLAN.md` for the sweep design and invocation.

## Deliverables (one per MODCOD)

- `curve_<rate>_<mod>.csv` — the FER/BER sweep.
- `curve_<rate>_<mod>.png` — simulated FER vs the ETSI TS 102 831
  QEF anchor.
- `tracing_<rate>_<mod>.jsonl` — structured `tracing` events
  (per-SNR completion + per-1000-frame heartbeats), tailable with `jq`.

(The per-config `curve_<slug>/` and `curve_<slug>_ext/` scratch dirs —
checkpoints, raw logs — are gitignored; the flat files above are the
committed artefacts.)

## Configuration

- Runner: `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` (issue
  `bbf6b6ee`), `Pipeline::dvb_t2` + `Scheduler::run_sweep_checkpointed`.
- Decoder: `--decoder sumproduct`; demap: `--demap exactlogmap`.
- GPU: `--gpu` on a `--features hip` build (LDPC BP + Gray-QAM demap
  offloaded to the HIP/ROCm device; CPU runs encode / interleave / AWGN /
  aggregation across the rayon pool).
- `--target-errors 100`, `--max-frames 1200000`, `--seed 42`.
- Driver: `run_campaign.sh` (main sweeps) + `run_extend.sh` (sub-10⁻⁴
  extension points). Resumable per SNR checkpoint.

## Host

- CPU: AMD Ryzen 9 5900X (12C / 24T)
- GPU: AMD Radeon RX 6950 XT (gfx1030, RDNA2)
- RAM: 31 GiB; OS: Linux 7.0.10-arch1-1; rustc 1.95.0
- RNG seed: 42

## Wall-clock

- Main six-config sweep: 2026-06-13 01:12 → 06:41 (~5 h 28 m).
- Sub-10⁻⁴ extension points (5 curves): 08:31 → 19:25 (~10 h 54 m;
  the host carried an external load average of ~28 on 24 threads during
  the extensions, which throttled the CPU-bound per-frame prep).
- Per-curve GPU-active time is in each CSV's `wall_seconds` column.

## Results — FER = 10⁻⁴ gap to ETSI TS 102 831 Table 44

| MODCOD     | ETSI QEF C/N | sim FER=10⁻⁴ crossing | gap (dB) | criterion |
|------------|:------------:|:---------------------:|:--------:|:---------:|
| 1/2 16-QAM | 6.0          | 6.167                 | **0.167**| ≤0.5 ✓    |
| 2/3 16-QAM | 8.9          | 9.017                 | **0.117**| ≤0.5 ✓    |
| 3/4 16-QAM | 10.0         | 10.224                | **0.224**| ≤0.5 ✓    |
| 1/2 64-QAM | 9.9          | 10.506                | **0.606**| ≤0.65 ✓   |
| 2/3 64-QAM | 13.5         | 14.085                | **0.585**| ≤0.65 ✓   |
| 3/4 64-QAM | 15.1         | 15.614                | **0.514**| ≤0.65 ✓   |

Crossing = log-linear interpolation of the two measured points
straddling FER = 10⁻⁴. Every curve has ≥ 100 frame errors at the
above-cliff bracketing point and ≥ 10⁶ frames at the deepest plotted
(sub-10⁻⁴) point.

### Why 16-QAM and 64-QAM differ

The ETSI TS 102 831 Table 44 anchors assume **"Genie-Aided" demapping**
(TS 102 831 §14.2, the paragraph immediately above Table 44):

> "The simulations include 'Genie-Aided' demapping ... Iterative
> demapping will approach this performance at low BERs and low-order
> constellations but **will be optimistic at higher BERs and for
> high-order constellations**."

Our chain uses real **single-pass** ExactLogMap BICM demapping (no
iterative demapping — that would be new code, out of scope; LDPC
decoder-algorithm changes are also out of scope). Single-pass demapping
is strictly below the genie bound, and the genie gap grows with
constellation order — exactly the measured pattern: ~0.12–0.22 dB for
16-QAM, ~0.51–0.61 dB for 64-QAM. The [hard] gap criterion was amended
accordingly (user-approved 2026-06-13): ≤0.5 dB for 16-QAM, ≤0.65 dB for
64-QAM. See `CLOSURE.md`.

## Reproduce

```bash
cargo build -p gf2-sim --release --features hip --bin dvb_t2_awgn_campaign
bash dev/benchmarks/dvb_t2_awgn/run_campaign.sh   # six main sweeps
bash dev/benchmarks/dvb_t2_awgn/run_extend.sh     # sub-1e-4 extension points
python3 dev/benchmarks/dvb_t2_awgn/finalize.py    # merge + plot + gap table
```

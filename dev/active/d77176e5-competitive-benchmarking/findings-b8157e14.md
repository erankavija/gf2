# Findings: layered i8 floor on DVB-T2 normal R1/2

Status: partial. Finished: the floor runs at 1.3, 1.5 and 2.0 dB (scales 4,
3, 2.5) and the waterfall runs at 1.00 to 1.20 dB (scales 4, 3.5, 3, 2.5, 2).
Not finished, stopped at the owner's wind-down: the wide-belief emulation on
the lost frames at 1.3 dB (killed during its first step, no result), the
per-frame failure run of scale 3, the message-limit split at scale 3, and the
wide-belief waterfall point. No wide-belief result exists.

The density-evolution step of REQ-03 is finished as an estimate: see
Density evolution (estimate) and Recommended rule (for REQ-03). The
measurements it lists are open.

Diagnosis step of @/issue/b8157e14; prior evidence in @/issue/d5cd53e0
(Findings, rework 1, REQ-03) and the default curve in @/issue/259acf37.

## Provenance

Every figure is an **indication**. Host fraktaali (AMD Ryzen 9 5900X), cores
20-23, 4 workers, `RAYON_NUM_THREADS=4 taskset -c 20-23 nice -n 10`, shared
host. One-minute load at step start and end: 47.8 / 14.8 (floor 1.3 dB), 28.3
/ 23.2 (1.5 dB), 23.2 / 20.3 (2.0 dB), 13.6 to 28.3 over the waterfall steps;
the load is other jobs, unattributed. FER is a count statistic; no timing is
stated.

- Harness `experiments/aff3ct-ldpc-gaps/src/bin/dvb_t2_fer_diag.rs` at
  `d36f5b540`, `cargo build --release -p aff3ct-ldpc-gaps --bin
  dvb_t2_fer_diag`, executable SHA-256
  `d92bbee4ced3737f48abe12690b58f53db68986a070080118ceee9ba14ff1ae5`.
- Command: `target/release/dvb_t2_fer_diag --ebn0 <p> --frames <F> --seed
  <S> --workers 4 --decoder layered:25 --decoder layered:25:7:<s>:31:40 ...`.
  All decoders of one point decode the same frames. Seeds: floor 5013, 5015,
  5020; waterfall 5100, 5105, 5110, 5115, 5120 (1.00 to 1.20 dB).
- Spec `layered:25:7:s:31:40` is NMS 7/8, cap 25, cap guard, channel limit 31,
  message limit 40, extrinsic (v→c) bound 87; `layered:25` is
  `LayeredConfig::dvb_t2()` (s = 4). Intervals are the harness's 95% Wilson
  intervals. Every row has 0 undetected errors.
- Logs stay outside the tracked tree, in the `target/b8157e14/` directory of
  the `worktree-agent-b8157e14` checkout: `target/b8157e14/floor.log`,
  `target/b8157e14/wf.log`, the partial `target/b8157e14/wide.log`, and the
  emulation source `target/b8157e14/b8157e14_emu.rs`.

## Floor region, 1 000 000 frames per point

| scale | 1.3 dB FE, FER [95%] | 1.5 dB FE, FER [95%] | 2.0 dB FE, FER [95%] |
|---|---|---|---|
| 4 (`dvb_t2`) | 138, 1.38e-4 [1.17e-4, 1.63e-4] | 64, 6.40e-5 [5.01e-5, 8.17e-5] | 22, 2.20e-5 [1.45e-5, 3.33e-5] |
| 3 | 27, 2.70e-5 [1.86e-5, 3.93e-5] | 7, 7.0e-6 [3.39e-6, 1.45e-5] | 2, 2.0e-6 [5.49e-7, 7.29e-6] |
| 2.5 | 35, 3.50e-5 [2.52e-5, 4.87e-5] | 2, 2.0e-6 [5.49e-7, 7.29e-6] | 1, 1.0e-6 [1.77e-7, 5.67e-6] |

Mean beliefs at ±127 per frame at 1.3 / 2.0 dB: 153 / 614 (s 4), 2.3 / 39.7
(s 3), 0.0 / 1.0 (s 2.5).

## Waterfall

| scale | 1.00 dB (20 000) | 1.05 dB (40 000) | 1.10 dB (100 000) | 1.15 dB (100 000) | 1.20 dB (100 000) | $10^{-3}$ crossing |
|---|---|---|---|---|---|---|
| 4 (`dvb_t2`) | 1.59e-1 | 1.97e-2 [1.84e-2, 2.11e-2] | 1.49e-3 [1.27e-3, 1.75e-3] | 4.40e-4 [3.28e-4, 5.91e-4] | 3.20e-4 [2.27e-4, 4.52e-4] | 1.116 dB |
| 3.5 | 1.36e-1 | 1.61e-2 [1.49e-2, 1.74e-2] | 1.09e-3 [9.04e-4, 1.32e-3] | 2.80e-4 [1.94e-4, 4.05e-4] | 1.20e-4 [6.87e-5, 2.10e-4] | 1.103 dB |
| 3 | 1.30e-1 | 1.62e-2 [1.50e-2, 1.75e-2] | 1.18e-3 [9.86e-4, 1.41e-3] | 2.20e-4 [1.45e-4, 3.33e-4] | 7.0e-5 [3.39e-5, 1.45e-4] | 1.105 dB |
| 2.5 | 1.67e-1 | 2.39e-2 [2.24e-2, 2.54e-2] | 2.20e-3 [1.93e-3, 2.51e-3] | 4.50e-4 [3.36e-4, 6.02e-4] | 1.20e-4 [6.87e-5, 2.10e-4] | 1.125 dB |
| 2 | 4.40e-1 | 1.14e-1 | 1.62e-2 | 2.41e-3 [2.13e-3, 2.73e-3] | 6.10e-4 [4.75e-4, 7.83e-4] | 1.182 dB |

Crossings are log-linear between the bracketing points. The default crosses
at 1.116 dB here against 1.128 dB in the window curve of @/issue/259acf37
(other seeds and frame counts). The default's curve already flattens between
1.15 and 1.20 dB.

## Reading

- **Scale 3 removes most of the floor and costs no waterfall.** Against the
  default on the same frames it loses 5 to 11 times fewer frames at 1.3, 1.5
  and 2.0 dB (non-overlapping intervals at every point) and crosses
  $10^{-3}$ 0.011 dB earlier. Scale 3.5 crosses at the same place; its floor
  was not run.
- **Scale 2.5 costs waterfall.** At 1.10 dB it loses 220 frames against 149
  for the default on the same frames, and crosses 0.009 dB later; it fails
  REQ-02 on this run. Scale 2 crosses 0.066 dB later.
- **REQ-01 is not met by any tested scale at 1.3 dB.** Scale 3 gives
  2.7e-5 and scale 2.5 3.5e-5 there, with almost no saturated beliefs. Whether
  that residual is a floor of the i8 decoder or the cap-25 min-sum waterfall
  tail is open: the scale-3 failure run, whose re-decodes under f32 flooding
  NMS 0.875 and SPA would decide it, did not finish. At 1.5 dB and 2.0 dB scale
  3 is at or below 1e-5 (upper interval ends 1.45e-5 and 7.3e-6).

## Falsified hypotheses

- "A scale between 2 and 4 removes the floor without waterfall cost"
  (@/issue/b8157e14, owner decision), as a statement about the whole range:
  falsified for 2.5 and 2 by the 1.10 dB row (2.20e-3 and 1.62e-2 against
  1.49e-3). Supported for 3, short of REQ-01 at 1.3 dB.
- "Removing belief saturation removes the floor": falsified as a sufficient
  condition. Scale 2.5 has a mean of 0.0 saturated beliefs per frame at 1.3 dB
  and still loses 35 of 1 000 000 frames, more than scale 3 (27, 2.3
  saturated). Saturation explains the default's excess, not the residual.

## Wide belief type (estimate, not measured)

A scalar emulation with an unbounded extrinsic bound (i16 beliefs, i8
messages) reproduced the library's i8 posteriors and iteration counts bit for
bit on 256 frames at 1.15 dB at the i8 bounds; its lost-frame run was killed
before any result. The change it needs:

- `gf2-kernels-simd` `ldpc_layered`: `apps` becomes `&mut [i16]`, messages
  may stay `i8` and widen on load; `LayerParams::extrinsic_limit` becomes
  `i16::MAX - message_limit`; `LAYER_SATURATION` becomes `i16::MAX`.
- AVX2 kernel: `epi8` min/max/abs/compare/blend on beliefs become `epi16`,
  16 lanes per 256-bit vector instead of 32. `movemask_epi8` sign extraction
  needs a pack or `epi16` mask step, and edge indices fit in `i16` lanes.
- `crates/gf2-coding/src/ldpc/layered.rs`: posterior, buffer and run copies widen to `i16`; `posterior()`
  returns `&[i16]`; `pack_signs` needs an `i16` variant; the contract page's
  State clause changes.
- Cost (estimate): twice the vectors per layer and twice the belief bytes
  moved, so about 0.5 to 0.6 times the i8 decoder's throughput per core. A
  360-check DVB-T2 layer takes 23 vectors instead of 12.

## Density evolution (estimate)

Every figure in this section is an **estimate** from offline density evolution
(DE), not a measurement. DE is asymptotic in block length and assumes a
cycle-free graph; it says nothing directly about trapping sets.

### Method

- **Model.** Exact discrete DE over integer message pmfs (float64, no
  sampling, no seed) of the decoder as `crates/gf2-kernels-simd/src/ldpc_layered.rs`
  and `crates/gf2-coding/src/ldpc/layered.rs` quantize it. BPSK AWGN, all-zero
  codeword, natural-log channel LLR $x \sim \mathcal N(\mu, 2\mu)$ with
  $\mu = 4R\,E_b/N_0$. Channel value $\mathrm{sign}(x)\min(\mathrm{round}(|x|s), C)$,
  rounding half away from zero. v→c value $v = \mathrm{clamp}(ch + \sum_{\text{others}} u, \pm E)$
  with $E = 127 - M$. c→v magnitude $\min(\lfloor 7m/8 \rfloor, M)$ of the
  excluded minimum $m$ (6/8 for NR). A bit is in error when $ch + \sum u < 0$,
  half when it is 0. The sign given to a zero value never reaches a message,
  since a zero input makes the excluded minimum 0. The fixed frame scale is
  used throughout, which is the regime of every lost frame (all at scale 4).
- **DVB-T2 normal R1/2 ensemble.** A two-edge-type DE taken from
  `crates/gf2-coding/src/ldpc/dvb_t2/table_a_1_R1_2_N64800.txt`. Every check
  has 5 information edges and 2 staircase edges. Information edges are 64% on
  degree-8 and 36% on degree-3 columns, and every parity column has degree 2.
  The single degree-1 column and the parity wrap are ignored. The plain
  $(\lambda, \rho)$ ensemble is not used: see Validation.
- **Layered schedule.** Each check gets an independent uniform position in a
  sweep of $K$ bins. Messages from earlier bins are current and the rest are
  from the previous sweep. $K = 90$, the code's number of check groups, is the
  primary column. $K$ sets the absolute threshold (1.112, 1.056, 1.030, 1.013
  and 1.006 dB at $K$ = 4, 8, 16, 32 and 90, scale 4). For $K \ge 8$ it
  changes scale differences by at most 0.006 dB: $s_3 - s_4$ is 0.009, 0.005, 0.009 and
  0.008 dB, and $s_{2.5} - s_4$ is 0.035, 0.031, 0.030 and 0.028 dB, at
  $K$ = 8, 16, 32 and 90.
- **NR BG1.** Protograph DE of the `crates/gf2-coding/src/ldpc/nr_5g/bg1.rs`
  base matrix, with all 68 columns transmitted ($R = 22/68$, the mother code of
  `crates/gf2-coding/tests/ldpc_layered_fer.rs`). The layered schedule runs in base-row order at
  `LayeredConfig::nr_5g()` (NMS 6/8, $C$ 23, $M$ 24, $E$ 103).
- **Criteria.** The threshold is the smallest $E_b/N_0$ at which the bit error
  probability $P_b$ falls below $10^{-6}$ within the iteration cap. Bisection
  tolerance is 0.005 dB for DVB-T2 and 0.01 dB for NR. "Cap ∞" means 3 000
  sweeps. The fixed point $P_{fp}$ is $P_b$ when it stops falling: less than
  0.1% gain over 100 sweeps (DVB-T2, $K$ = 8) or over 60 sweeps (NR).
- **Configurations.** "i8" is the decoder's bounds. "Wide" is $C$ 200, $M$ 400,
  $E$ 800: the same quantum without saturation.
- **Scripts.** `target/b8157e14/de/qde.py` (the DE), `target/b8157e14/de/codes.py`
  (the code tables) and the `job*` run scripts with their logs, in the same
  directory of the `worktree-agent-b8157e14` checkout, outside the tracked
  tree.

### Validation

- **Regular (3,6), NMS 0.75, flooding, cap 200, target $10^{-4}$.** Unquantized
  limit: 16 quanta per nat and ±40 nat bounds. The result is 1.212 dB. The
  sample DE of @/issue/3461a52a gives 1.20 dB; its ±0.05 dB precision holds
  this.
- **(3,6) plain min-sum, cap 1 000.** 1.703 dB ($\sigma$ = 0.822). This is
  near the min-sum threshold as recalled from the literature. The recalled
  value is not verified against a source.
- **DVB-T2 R1/2 with the plain $(\lambda,\rho)$ ensemble, NMS 0.875,
  flooding, target $10^{-4}$.** 1.191 dB at cap 50 and 1.051 dB at cap 200,
  against 1.17 and 0.96 dB from the @/issue/3461a52a scripts. Cap 50 agrees.
  Cap 200 differs by 0.09 dB, outside that issue's stated ±0.05 dB.
  - The cause is the ensemble model. Under $(\lambda,\rho)$ the degree-2
    subgraph branches with $\lambda_2\rho'(1) = 1.71 > 1$. The staircase makes
    it a path, so each check passes a degree-2 error to exactly one other
    degree-2 bit.
  - The $(\lambda,\rho)$ DE keeps a fixed point at $P_b$ $4\times10^{-5}$ to
    $5\times10^{-4}$ (1.0 to 1.6 dB) at any bounds, near the $10^{-4}$ target.
    Thresholds taken at that target therefore move with sampling noise. The
    two-edge-type DE has no such fixed point.
  - This bears on the DVB-T2 thresholds of @/issue/3461a52a, which use the
    $(\lambda,\rho)$ ensemble. It is reported here and not reworked.

### Waterfall: threshold versus scale (DVB-T2 normal R1/2)

Threshold $E_b/N_0$ in dB; $m$ is the mean channel magnitude at the threshold
(2.82 nat at 1.006 dB).

| scale | layered cap 25, $K$ 90, i8 | layered cap 25, $K$ 8, i8 | same, wide | flooding cap 50, i8 | layered cap ∞, $K$ 8, i8 | same, wide |
|---|---|---|---|---|---|---|
| 2 | 1.092 | 1.155 | 1.155 | 1.078 | 0.807 | 0.807 |
| 2.25 | | 1.116 | | | | |
| 2.5 | 1.034 | 1.091 | 1.091 | 1.022 | 0.790 | 0.790 |
| 2.75 | | 1.078 | | | | |
| 3 | 1.014 | 1.065 | 1.065 | 1.000 | 0.798 | 0.798 |
| 3.25 | | 1.061 | | | | |
| 3.5 | 1.006 | 1.056 | 1.056 | 0.992 | 0.811 | 0.811 |
| 4 | 1.006 | 1.056 | 1.052 | 0.996 | 0.820 | 0.820 |
| 5 | 1.002 (wide; wide at scale 4: 1.002) | 1.185* | 1.048 | 1.164* | 1.159* | 0.837 |
| 6 | | none* | 1.052 | none* | none* | |

\* The i8 fixed point at that scale ($P_{fp}$ ≥ $10^{-6}$, table below) lies at
the $10^{-6}$ target, so these entries measure the floor, not the waterfall.

**Reading.**

- At cap 25 the threshold is a function of the quantum alone. i8 and wide
  agree to 0.004 dB for $s \le 4$.
- The resolution cost against scale 3.5 is 0.008 dB at scale 3, 0.028 dB at
  2.5 and 0.086 dB at 2 ($K$ = 90).
- In units of the frame statistic the cap guard already computes, the cost
  stays at or below 0.01 dB while $Q = s\,m \ge 8.5$ quanta at the threshold
  (scale 3). This is $s\mu \ge 7.6$ in channel-mean quanta.
- At cap ∞ the ordering inverts slightly: scale 2.5 is best, by 0.03 dB. The
  cap-25 column is the one REQ-02 concerns.

**NR BG1 mother code, layered cap 25.**

- Thresholds are 0.391, 0.338, 0.320, 0.320 and 0.320 dB at scales 2, 2.5, 3,
  3.5 and 4 ($m$ = 1.77 nat).
- Its resolution bound is lower: the cost is 0 at $Q = 5.3$ (scale 3) and
  0.018 dB at $Q = 4.4$ (scale 2.5).
- $Q_{wf}$ is therefore a DE output of the code, not a universal constant.

### Floor side: DE fixed point versus scale and bounds

DVB-T2 normal R1/2, layered $K$ = 8 to convergence, $P_{fp}$ at 1.1 / 1.3 / 1.5 / 2.0 dB:

| configuration | s 2 | s 2.5 | s 3 | s 3.5 | s 4 | s 5 | s 6 |
|---|---|---|---|---|---|---|---|
| i8 ($C$ 31, $M$ 40, $E$ 87) | 7.0e-9 / 2.7e-9 / 1.1e-9 / 1.2e-10 | 7.0e-9 / 3.5e-9 / 1.8e-9 / 4.0e-10 | 1.6e-8 / 9.8e-9 / 6.0e-9 / 1.9e-9 | 4.8e-8 / 3.2e-8 / 2.2e-8 / 9.2e-9 | 1.5e-7 / 1.1e-7 / 7.7e-8 / 3.8e-8 | 1.1e-6 / 8.4e-7 / 6.6e-7 / 3.7e-7 | 5.4e-6 / 4.3e-6 / 3.4e-6 / 2.0e-6 |
| $M$ 48, $E$ 79 | 6.7e-9 / 2.3e-9 / 8.2e-10 / 5.9e-11 | 3.6e-9 / 1.5e-9 / 6.5e-10 / 8.9e-11 | 4.8e-9 / 2.5e-9 / 1.3e-9 / 2.9e-10 | 1.0e-8 / 6.2e-9 / 3.8e-9 / 1.2e-9 | 2.7e-8 / 1.8e-8 / 1.2e-8 / 4.6e-9 | 1.8e-7 / 1.3e-7 / 9.6e-8 / 4.7e-8 | 8.8e-7 / 6.7e-7 / 5.2e-7 / 2.8e-7 |
| $M$ 32, $E$ 95 | 1.3e-8 / 6.3e-9 / 3.2e-9 / 6.4e-10 | 3.1e-8 / 1.9e-8 / 1.2e-8 / 3.9e-9 | 1.1e-7 / 7.5e-8 / 5.3e-8 / 2.3e-8 | 3.9e-7 / 2.9e-7 / 2.2e-7 / 1.2e-7 | 1.3e-6 / 1.0e-6 / 7.9e-7 / 4.5e-7 | 9.7e-6 / 7.8e-6 / 6.3e-6 / 3.9e-6 | 4.7e-5 / 3.7e-5 / 3.0e-5 / 1.9e-5 |
| $M$ 120, $E$ 400 ($C$ 31) | 6.7e-9 / 2.3e-9 / 8.0e-10 / 5.0e-11 | 3.1e-9 / 1.1e-9 / 3.9e-10 / 2.5e-11 | 1.9e-9 / 6.9e-10 / 2.4e-10 / 1.6e-11 | 1.4e-9 / 4.9e-10 / 1.8e-10 / 1.1e-11 | 1.1e-9 / 3.9e-10 / 1.4e-10 / 9.3e-12 | 7.9e-10 / 3.0e-10 / 1.1e-10 / 9.6e-12 | 8.1e-10 / 3.4e-10 / 1.5e-10 / 1.9e-11 |

**Readings (estimates).**

- **The governing variable is $r = M/(s\,m)$.** This is the message limit in
  units of the frame's mean channel magnitude. Rows with different $M$ fall on
  one curve of $r$ within a factor of 2. For example, $r$ = 2.79 at 1.1 dB
  gives 1.1e-6 ($s$ 5, $M$ 40), 1.3e-6 ($s$ 4, $M$ 32) and 8.8e-7 ($s$ 6,
  $M$ 48).
- **$E$ does not enter in DE, and $C$ barely does.** $E$ = 400 at $M$ 40
  reproduces the i8 row to the printed digits. $C$ 23 lowers $P_{fp}$ by 7% at
  scale 4 and by at most 1.5 times (scale 6). On a tree a v→c value above
  $\lceil 8M/7 \rceil = 46$ already yields the saturated message $M$, so
  $E = 87$ never binds.
- **Without message saturation the fixed point is a slow residual.** With
  $M$ 120 it is $10^{-9}$ to $10^{-11}$; the staircase path is a critical
  (branching 1) error channel. It decreases with finer quantization.
- **The saturation part dominates while $r \lesssim 5.5$.** The i8 fixed point
  comes within a factor of about 2 of the $M$-120 residual near $r$ = 5.4 to
  6.0. At 2.0 dB, $64\,800\,P_{fp} \le 10^{-5}$ needs $r \ge 5.6$.
- **NR BG1 at i8.** $P_{fp}$ at 0.6 / 1.0 dB is:

  | scale | 2 | 2.5 | 3 | 4 | 5 |
  |---|---|---|---|---|---|
  | $P_{fp}$ | 6.0e-13 / 6.3e-13 | 8.9e-11 / 8.4e-11 | 2.3e-9 / 2.1e-9 | 1.4e-7 / 1.2e-7 | 1.8e-6 / 1.5e-6 |
  | $r$ at 1.0 dB | 6.0 | 4.8 | 4.0 | 3.0 | 2.4 |

  - $M$ 32 collapses onto the same $r$ curve: $s$ 4, $r$ 4.31 gives 1.95e-9,
    against 2.3e-9 for i8 at $s$ 3 and the same $r$.
  - With $M$ 400, $P_b$ falls below $10^{-15}$. NR's fixed point is therefore
    entirely saturation.
  - $26\,112\,P_{fp} \le 10^{-5}$ needs $r \ge 4.4$ to 4.8.

### Comparison with the measured indications

**Supported.**

- **Waterfall cost of a coarser quantum.** Relative to scale 3.5, DE predicts
  +0.008, +0.028 and +0.086 dB at scales 3, 2.5 and 2. The measured $10^{-3}$
  crossings give +0.002, +0.022 and +0.079 dB.
- **Scale 4 against 3.5.** DE puts them at the same threshold. The measured
  0.013 dB later crossing of scale 4 is the start of its floor at $10^{-3}$,
  which an asymptotic threshold does not see.
- **Absolute thresholds.** The DE threshold (1.006 dB) lies 0.10 dB below the
  measured crossing, the finite-length gap.
- **Floor ordering across scales.** Ratios of $P_{fp}$ (DE) against ratios of
  measured lost frames:

  | point | scale 4 / scale 3, DE | measured | scale 3 / scale 2.5, DE | measured |
  |---|---|---|---|---|
  | 1.3 dB | 10.9 | 5.1 | 2.8 | 0.77 |
  | 1.5 dB | 12.9 | 9.1 | 3.3 | 3.5 |
  | 2.0 dB | 19.4 | 11 | 4.8 | 2 |

  The 1.3 dB inversion for scale 2.5 is its waterfall tail: its threshold is
  0.020 dB above scale 3's.
- **Scale 2 with $M$ 20.** It has the default's $r$ (3.36 at 1.3 dB). DE gives
  2.6e-7 against the default's 1.1e-7. The measured FER is 4.88e-4 against
  1.41e-4 (@/issue/d5cd53e0 range runs).
- **Lower message limit at scale 4.** $M$ 30 measured gives 27 times the
  default's FER at 1.3 dB. DE gives about 10 times, from $M$ 32.
- **The 1.3 dB residual of scale 3.** At $r$ = 4.47, DE's saturation fixed point
  is 14 times its $M$-120 residual. DE therefore attributes the residual mostly
  to message saturation, not to the cap-25 waterfall tail.

**Not supported: absolute floor level.** $n P_{fp}$ overestimates the measured
wrong bits per frame by 3 to 18 times.

- The measured figure is FER times 7.3 wrong bits per lost frame at 1.3 dB
  and 6.2 at 2.0 dB, from the scale-4 per-frame run of @/issue/d5cd53e0. The
  scale-3 rows assume the same count.
- At scale 4 the overestimate is 6.8 times at 1.3 dB and 18 times at 2.0 dB.
  At scale 3 it is 3.2 and 10 times.
- A finite graph absorbs into the all-correct state, which the tree does not
  have. DE ranks configurations; it does not predict the FER floor.

### Falsifications

- **DE prediction: raising $M$ at fixed width lowers the floor, and $E$ is
  irrelevant.** At scale 4, $M$ 48 / $E$ 79 should give 6 times fewer bit
  errors at 1.3 dB.
  - Contradicted by @/issue/d5cd53e0. `7:4:31:50` ($M$ 50, $E$ 77) measures
    FER 1.74e-1 at 1.3 dB. `7:4:31:60` recovers none of the lost frames.
    `7:2:31:83` ($E$ 44) measures 2.44e-2.
  - The tree model does not represent the layered decoder's stored belief. The
    kernel replaces a belief by $\mathrm{clamp}(app - u_{old}, \pm E) + u_{new}$,
    so a belief at the bound holds $E + M = 127$ quanta. A check that reverses
    its message on it leaves $E - M$.
  - The belief keeps at least one message's worth of the right sign only if
    $E - M \ge M$, that is $E \ge 2M$ or $3M \le 127$.
  - Every measured split that works ($M$ 20 at scale 2, $M$ 30, $M$ 40)
    satisfies it. Every split that violates it collapses ($M$ 50, 60, 83).
    $M$ 20 at scale 4 also collapses, but through the DE side: $r$ = 1.7.
  - This bound comes from the saturation argument and the measured splits, not
    from DE.
- **Earlier reading: the rule keeps NR BG1 unchanged to its $10^{-3}$
  crossing.** The partial diagnosis computed $E/(\kappa m) = 103/(10 \times
  1.84) = 5.6$ at the 0.559 dB crossing, above the base scale 4, and concluded
  that NR keeps scale 4 there.
  - Contradicted by the density evolution above (estimate). The floor is
    governed by $r = M/(s\,m)$, not by $E$. At scale 4, $r$ = 3.3 against
    $\kappa_M \approx 4.6$, so DE predicts a saturation floor at the default
    scale and a rule scale of about 2.8 at the crossing.
  - Neither statement is measured. The NR BG1 floor run in the next steps
    decides between them.
- **@/issue/3461a52a's DVB-T2 cap-200 threshold of 0.96 dB.** Its
  $(\lambda,\rho)$ model is contradicted by the staircase structure, as stated
  under Validation. The NMS-factor conclusion of that issue is not re-tested
  here.

### Literature

Read levels are those of the literature check for this section. "Metadata
only" means the bibliographic record and abstract.

- **Choosing the step by quantized DE.** A quantized-DE threshold minimization
  over the step gives a numerical procedure per (profile, rate, width), not a
  closed form. It optimizes the waterfall only.
  - E. Ben Yacoub, "Matched quantized min-sum decoding of LDPC codes", arXiv:2010.07538,
    doi:10.1109/ITW46852.2021.9457667. Full text. The step is chosen "to
    minimize the decoding threshold".
  - J. Chen, A. Dholakia, E. Eleftheriou, M. P. C. Fossorier, X.-Y. Hu, IEEE
    TCOM 53(8), 2005, doi:10.1109/TCOMM.2005.852852. Metadata only. DE fixes
    the key parameters and evaluates "finite quantization effects".
- **Step against saturation at a fixed width.**
  - X. Zhang, P. H. Siegel, IEEE TCOM 62(1), 2014, doi:10.1109/TCOMM.2013.112313.120917,
    arXiv:1212.6465. Full text. A "smaller step size produces better
    performance in the waterfall region but a higher error floor", and floors
    "monotonically decrease as the saturation levels increase".
  - Z. Zhang, L. Dolecek, B. Nikolić, V. Anantharam, M. J. Wainwright,
    doi:10.1109/TCOMM.2009.11.080105, 2009. Full text. The masthead reads IEEE
    Trans. Wireless Commun. 8(11) while Crossref lists IEEE Trans. Commun.
    57(11). An increase in range fixes floors more effectively than an
    increase in resolution.
  - J. Zhao, F. Zarkeshvari, A. H. Banihashemi, IEEE TCOM 53(4), 2005,
    doi:10.1109/TCOMM.2004.836563. Metadata only. Four bits suffice in the
    waterfall.
- **Saturation as the cause of floors.**
  - B. K. Butler, P. H. Siegel, IEEE TIT 60(12), 2014, doi:10.1109/TIT.2014.2363832,
    arXiv:1202.2826. Full text. Raising the LLR limits lowers the floor by
    orders of magnitude and changes the dominant trapping set.
  - B. Kudekar, T. Richardson, A. Iyengar, IEEE TIT, 2017, doi:10.1109/TIT.2017.2723484,
    arXiv:1412.8090. Metadata only. Saturation degrades the threshold
    gracefully but affects stability strongly, and low-degree variables are the
    sensitive point.
- **Layered schedules.** Layered decoding needs about half the iterations near
  threshold, and this is proved only for the BEC. Both sources are metadata
  only.
  - E. Sharon, S. Litsyn, J. Goldberger, IEEE TIT 53(11), 2007, doi:10.1109/TIT.2007.907507.
  - J. Zhang, M. Fossorier, IEEE TCOM 53(2), 2005, doi:10.1109/TCOMM.2004.841982.
- **What the sources do not give.** None gives a closed-form step rule tied
  to the channel LLR mean. J. Max, IRE Trans. IT 6(1), 1960,
  doi:10.1109/TIT.1960.1057548 (metadata only), is MSE-optimal, which is not a
  decoding criterion.

DE here reproduces the Zhang–Siegel trade-off in closed variables: the
waterfall sets a lower bound on $s\,m$ and the message saturation sets an upper
bound on $s\,m/M$.

## Recommended rule (for REQ-03)

**Rule.** The frame's mean channel magnitude in quanta, $Q = s_f\,m$, is the
controlled quantity. Here $m$ is the frame's mean magnitude in nat, as the cap
guard computes it:

$$s_f = \min\left(b,\ \frac{C}{g\,m},\ \frac{Q}{m}\right),\qquad
Q = \begin{cases} M/\kappa_M & \text{if } M/\kappa_M \ge Q_{wf},\\ Q_{wf} & \text{otherwise,}\end{cases}
\qquad 3M \le 127\ (E \ge 2M).$$

It stays per frame and caller-overridable as the cap guard is. For DVB-T2,
$C/(g\,m) = 15.5/m$ exceeds $Q/m$, so the guard term does not bind.

**Constants and their sources.**

| constant | value | source |
|---|---|---|
| $b, C, g$ | 4, 31, 2 | `LayeredConfig`; unchanged |
| $M$ | at most 42 at i8 | the split bound $E \ge 2M$ with $E = 127 - M$: saturation argument and @/issue/d5cd53e0 splits, not DE. NR BG1, with columns up to degree 30, needs a larger multiple (`nr_5g` rustdoc: $M$ 31 keeps a floor, so $E \ge 3.1M$ fails there; $M$ 24 gives $E$ = 4.3M). The multiple is a function of column degree that is not derived here. |
| $Q_{wf}$ | 8.5 (DVB-T2 R1/2), 4.4 to 5.3 (NR BG1 mother) | DE: smallest $s\,m$ at the cap-25 threshold whose resolution cost is at most 0.01 dB, from the code's degree structure, normalization and cap |
| $\kappa_M$ | 5.4 to 6.0 (DVB-T2 R1/2), 4.4 to 4.8 (NR BG1) | DE: smallest $r = M/(s\,m)$ at which the saturation fixed point reaches the unsaturated one (within 2 times), or $n P_{fp} \le 10^{-5}$ where that residual is zero |

**Consequences (estimates).**

- **DVB-T2 R1/2 at i8 has no window.** $M/\kappa_M = 40/5.7 \approx 7.0$ is
  below $Q_{wf} = 8.5$. Even $M = 42$ gives 7.4. The rule therefore takes
  $Q = Q_{wf}$, which keeps REQ-02 and leaves a DE saturation floor about 5 to
  16 times the unsaturated residual.
  - This is consistent with scale 3 missing REQ-01 at 1.3 dB.
  - $s_f = 8.5/m$ gives 2.96 at 1.11 dB, 2.85 at 1.3 dB, 2.74 at 1.5 dB and
    2.49 at 2.0 dB.
- **The DE constant matches the measured cell.** The indication-backed rule
  $E/(\kappa m)$ with $\kappa \approx 10$ has the same $1/m$ form, with
  $Q = 87/10 = 8.7$ (scale 3.03 at 1.1 dB). DE recovers that constant from
  structure as $Q_{wf}$.
  - At a fixed split, the $E$ and $M$ framings are proportional:
    $\kappa_E = \kappa_M\,E/M$. Measured scale sweeps cannot separate them.
  - DE places the floor mechanism on $M$, and the split runs place a second,
    non-DE bound on $E$.
  - The measured $\kappa_E$ values (7.6, 8.7, 10.1, 12.1 for scales 4, 3.5, 3,
    2.5 at $m$ = 2.87) stay as the measured calibration of that form.
- **Window condition at width $B$.** A window exists only if
  $B/(k+1) \ge \kappa_M Q_{wf}$, with split multiple $k$ ($k$ = 2 for DVB-T2).
  For DVB-T2 R1/2 this needs $B \ge 3 \times 5.7 \times 8.5 \approx 145$, so i8
  beliefs (127) fall short.
- **Wider beliefs.** With i16 beliefs and i8 messages, the split bound leaves
  $M$ free up to 126. The rule then predicts scale 4 with
  $M \ge \kappa_M\,s\,m = 5.7 \times 4 \times 2.82 \approx 64$. That keeps the
  scale-4 waterfall and puts the saturation floor at the residual.
- **NR BG1 prediction.** DE predicts a saturation floor at the default scale 4:
  $r$ = 3.3 at the 0.559 dB crossing, against $\kappa_M \approx 4.6$.
  - A window exists at about the 0.01 dB bound: $M/\kappa_M = 24/4.6 = 5.2$
    against $Q_{wf}$ = 4.4 to 5.3.
  - The rule lowers NR's scale to about $5.2/m$, which is 2.8 at the crossing,
    at a DE waterfall cost of at most 0.02 dB.
  - This reverses the earlier reading that the rule keeps NR unchanged. That
    reading rested on $E$ (103), which DE shows does not govern the floor.
- **Higher DVB-T2 rates.** A larger $m$ at the operating point lowers $s_f$
  through the same $Q/m$. The resolution bound is again in $Q$, so the rule
  needs no per-rate scale table, only per-code $Q_{wf}$ and $\kappa_M$ from DE.

**Basis of each part.**

- *DE*: that $r = M/(s\,m)$ governs the floor-side fixed point, $Q_{wf}$,
  $\kappa_M$, and the ordering of scales.
- *Saturation argument and measured splits*: the bound $E \ge 2M$, and that $E$
  matters at all.
- *Measurement only*: the absolute FER floor. DE overestimates the wrong-bit
  rate 3 to 18 times.

**Required next steps.** Measurements 1 to 6 are for the lead to schedule;
step 7 is offline DE.

1. DVB-T2 normal R1/2: the per-frame rule $s_f = 8.5/m$ (the cap guard with
   the $Q/m$ term). Run a waterfall at 1.00 to 1.20 dB and a floor at 1.3, 1.5
   and 2.0 dB with $10^6$ frames per floor point, against `dvb_t2()` on the
   same frames (REQ-01, REQ-02).
2. Scale 3 per-frame failure run at 1.3 dB with f32 flooding NMS 0.875 and SPA
   re-decodes. DE predicts the residual is saturation-driven; a waterfall-tail
   residual would contradict it.
3. Split edge at scale 3: $M$ 36, 40, 42 ($E \ge 2M$) and 44, 46 ($E < 2M$) at
   1.3 and 1.5 dB. DE predicts $M$ 42 below $M$ 40 (6.4e-9 against 9.8e-9);
   the split bound predicts degradation from $M$ 44.
4. NR BG1 Z = 384 mother code: scale 4 against 2.8 at 1.0 to 2.3 dB, down to
   FER $10^{-6}$, plus the waterfall to $10^{-3}$. This tests the NR saturation
   floor DE predicts.
5. Wide-belief emulation (i16 beliefs) at scale 4, $M$ 64, on the 1.3 dB lost
   frames and at the waterfall.
6. Wrong bits per lost frame at scale 3, to recalibrate the $n P_{fp}$
   comparison.
7. Offline DE for $Q_{wf}$ and $\kappa_M$ of the other DVB-T2 rates (normal and
   short), with the multi-edge model, and of NR BG2.

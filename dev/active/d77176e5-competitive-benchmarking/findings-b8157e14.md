# Findings: layered i8 floor on DVB-T2 normal R1/2

Status: partial. Finished: the floor runs at 1.3, 1.5 and 2.0 dB (scales 4,
3, 2.5) and the waterfall runs at 1.00 to 1.20 dB (scales 4, 3.5, 3, 2.5, 2).
Not finished, stopped at the owner's wind-down: the wide-belief emulation on
the lost frames at 1.3 dB (killed during its first step, no result), the
per-frame failure run of scale 3, the message-limit split at scale 3, and the
wide-belief waterfall point. No wide-belief result exists.

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
  `dab7098ba`, `cargo build --release -p aff3ct-ldpc-gaps --bin
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
  the `worktree-agent-b8157e14` checkout: `floor.log`, `wf.log`, the partial
  `wide.log`, and the emulation source `b8157e14_emu.rs`.

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
- `layered.rs`: posterior, buffer and run copies widen to `i16`; `posterior()`
  returns `&[i16]`; `pack_signs` needs an `i16` variant; the contract page's
  State clause changes.
- Cost (estimate): twice the vectors per layer and twice the belief bytes
  moved, so about 0.5 to 0.6 times the i8 decoder's throughput per core. A
  360-check DVB-T2 layer takes 23 vectors instead of 12.

## Recommended rule (for REQ-03; to be confirmed)

The scale enters only through the channel mean in quanta, since the bounds
are fixed in quanta. Keep the extrinsic bound $E = 127 - M$ at least $\kappa$
times the quantized mean channel magnitude $s_f m$, where $m$ is the frame's
mean magnitude in nat as the cap guard computes it, capped at the base scale:

$$s_f = \min\left(b,\ \frac{C}{g\,m},\ \frac{E}{\kappa\,m}\right).$$

This is the existing cap guard with a second term, so it adapts per frame and
stays caller-overridable. At the crossing ($m$ = 2.87 nat for BPSK AWGN at
1.11 dB) $\kappa$ = 7.6, 8.7, 10.1, 12.1 for scales 4, 3.5, 3, 2.5: the floor
falls from 7.6 to 10.1 and the waterfall cost starts at 12.1, so
$\kappa \approx 10$ (an indication-backed value, cited as an example). With
$\kappa = 10$ the rule gives 3.03 at 1.1 dB, 2.92 at 1.3 dB, 2.81 at 1.5 dB
and 2.54 at 2.0 dB, inside the tested range at every point; that adaptive
form itself was not run.

**NR BG1** (`LayeredConfig::nr_5g()`, $E$ = 103): at its crossing (0.559 dB,
mother-code rate 22/68) $m$ = 1.84 nat, and $E/(10 m)$ = 5.6 exceeds the base
scale 4, so the rule keeps NR unchanged to its $10^{-3}$ crossing; it lowers
NR's scale only above $m$ = 2.58 nat (about 2.3 dB). Its mean in quanta at the
crossing, 7.4, matches the 8.6 of DVB-T2 scale 3.

Open before adoption: the floor and waterfall of the adaptive rule itself; the
higher DVB-T2 rates, where a large $m$ lowers the scale and coarsens the
quantum; the 1.3 dB residual; the wide-belief alternative.

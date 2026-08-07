# DVB-T2 AWGN campaign — closure note (e4849f07 / epic 2928ccce)

Date: 2026-06-13. Decoder: SumProduct + ExactLogMap. GPU: AMD Radeon
RX 6950 XT (gfx1030), `gf2-sim` hybrid CPU+GPU pipeline. Seed 42.

## Per-curve gap to ETSI TS 102 831 Table 44 (QEF, BER=10⁻⁷ after LDPC)

The ETSI anchor is a single QEF C/N threshold (≈ FER 10⁻⁴; see the
reference TOML header). "sim crossing" is the log-linear interpolation
of the two measured points straddling FER = 10⁻⁴. "gap" = sim crossing −
ETSI anchor.

| MODCOD     | ETSI (dB) | sim crossing (dB) | gap (dB) | bracketing points (Es/N₀: FER, errors)            |
|------------|:---------:|:-----------------:|:--------:|---------------------------------------------------|
| 1/2 16-QAM | 6.0       | 6.167             | 0.167    | 6.15: 3.07e-4, 100  /  6.20: 1.08e-5, 13 (1.2M f) |
| 2/3 16-QAM | 8.9       | 9.017             | 0.117    | 9.00: 3.12e-4, 100  /  9.05: 1.00e-5, 12 (1.2M f) |
| 3/4 16-QAM | 10.0      | 10.224            | 0.224    | 10.20: 5.46e-4, 100 / 10.25: 1.67e-5, 20 (1.2M f) |
| 1/2 64-QAM | 9.9       | 10.506            | 0.606    | 10.50: 1.56e-4, 100 / 10.55: 4.17e-6, 5 (1.2M f)  |
| 2/3 64-QAM | 13.5      | 14.085            | 0.585    | 14.05: 7.17e-4, 100 / 14.10: 4.42e-5, 53 (1.2M f) |
| 3/4 64-QAM | 15.1      | 15.614            | 0.514    | 15.60: 2.10e-4, 100 / 15.65: 1.58e-5, 19 (1.2M f) |

## Criterion outcomes

- [hard] All 6 curves produced — **MET**.
- [hard] Frame-count / bracketing criterion, **amended 2026-06-13
  (user-approved)** to the epic's own operative parenthetical: the
  deepest plotted SNR point has ≥ 10⁶ frames, and the point bracketing
  FER = 10⁻⁴ from above (FER ≥ 10⁻⁴) has ≥ 100 frame errors — **MET** for
  all six (deepest plotted point = 1 200 000 frames each; above-cliff
  bracketing point ≥ 100 errors each). The original wording ("≥ 100
  errors at *each* bracketing point") was infeasible: the near-vertical
  N=64800 waterfall puts the sub-10⁻⁴ point at FER ~ 10⁻⁵, where ≥ 100
  errors would require 5–24 × 10⁶ frames per curve. See the e4849f07
  issue description for the amendment record.
- [hard] Gap to ETSI TS 102 831, **amended 2026-06-13 (user-approved)**:
  ≤ 0.5 dB for 16-QAM, ≤ 0.65 dB for 64-QAM — **MET** for all six
  (max 16-QAM 0.224; max 64-QAM 0.606).
- [aspirational] ≤ 0.3 dB at FER = 10⁻⁴ — **MET on all three 16-QAM
  curves** (0.117 / 0.167 / 0.224 dB). Not met on the 64-QAM curves
  (genie-aided-reference reason below); aspirational, not required.
- [hard] Artefacts committed under `dev/benchmarks/dvb_t2_awgn/`; PLAN.md
  and README.md present — **MET**.

## Amendment rationale (genie-aided demapping)

ETSI TS 102 831 §14.2, immediately above Table 44, verbatim:

> "The simulations include 'Genie-Aided' demapping (see clause
> 10.5.3.2.2). Iterative demapping will approach this performance at low
> BERs and low-order constellations but will be optimistic at higher BERs
> and for high-order constellations."

The Table 44 anchors are therefore an idealized lower bound. Our chain
uses real single-pass ExactLogMap BICM demapping — strictly below the
genie bound, with a loss that grows with constellation order. This is
the in-scope decoding: the epic forbids LDPC decoder-algorithm changes,
and ExactLogMap is already the most exact in-scope demapper; iterative
demapping (BICM-ID) would be new, out-of-scope code. The measured
pattern (16-QAM ~0.12–0.22 dB, 64-QAM ~0.51–0.61 dB) matches the spec's
own statement that the genie reference is optimistic for high-order
constellations. Per the project's "measurements, not guesses" rule, the
[hard] 0.5 dB target — written before empirical evidence — was amended
with the observed numbers and this rationale, with explicit user
approval, rather than reworked against a hypothesis.

## Verdict

All six curves meet the amended success criteria. e4849f07 and epic
2928ccce are closeable.

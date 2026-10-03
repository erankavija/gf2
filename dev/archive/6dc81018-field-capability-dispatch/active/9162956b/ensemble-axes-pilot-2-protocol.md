# Predeclared pilot 2: the translation enumeration

The first pilot of issue `9162956b`, predeclared in
[`ensemble-axes-pilot-protocol.md`](ensemble-axes-pilot-protocol.md), reads
**REJECT** on the axis set it tested: v1's three alignment axes with a
translation axis added reach neither coverage floor. The same pilot measures the
translation axis **on its own** clearing the floor at all thirty-four cells, and
measures why the combination does not: the alignment axes raise `.text`'s own
alignment and quantise the translation away.

This document fixes the second pilot, on the enumeration that follows from that
measurement. It is written before the second pilot's first timed window exists.
Pilot 1 stands as taken and is recorded with its numbers; this document does not
revise it, and its Group SH is evidence about an axis, not a decision under this
rule.

The pilot decides an **axis set**. It decides no verdict, measures no candidate
revision, and moves nothing v1 predeclares: τ_cell stays 5 %, τ_set stays 2 %,
the pinned set keeps its thirty-four cells, the schema token stays
`selector-non-regression-v1`, and the attribution margin stays three standard
errors.

## 1. The enumeration under test

Member `j`, for `j` from 0 to 255:

    E(j) = ⌊j / 2⌋                        G(j) = (E(j) + j) mod 2

| Axis | Option | Levels |
|---|---|---|
| E | `-C link-arg=-Wl,--build-id=0x<hexadecimal string of 2·(20 + 32·E) zeros>` | 0–127 |
| G | `-C link-dead-code` | 0–1 |

A level of 0 omits its option, so member 0 builds under empty `RUSTFLAGS` and is
the ordinary build. Member `j` records `--execution j+1`. **K = 256.**

v1 §3.1's axes A, B and C and v1 §6.6's axis D are not in this enumeration.

The construction is verified by enumeration before the pilot runs: 256 distinct
members; `E(j) + G(j)` congruent to `j` modulo 2 at every member, which is v1
§3.2's parity property in v1's own form; and the half-split on member-index
parity holding exactly one member at each level of E and sixty-four at each
level of G.

## 2. What the pilot measures

**The complete reference-arm ensemble**: all 256 members built at revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6` in
`.agents/worktrees/control-0c072d73`, each recording one execution of one
repetition at `--target-ms 250` over the pinned thirty-four cells, in one lock
session under `dev/scripts/ccx1-bench-flock.sh` with `GF2_BENCH=1`. `s_R` is
therefore the ensemble's own statistic at the K the amendment proposes, not a
subsample of it. No candidate revision is built or measured.

Two further groups run interleaved with it:

- **Group CTL**, thirty-two members of v1 §3.1's K = 128 enumeration —
  `j = 4i + (i mod 2)` for `i` from 0 to 31 — reusing pilot 1's staged binaries.
  It calibrates this session against pilot 1's.
- **Group FIX**, eight executions of **one** binary, the ordinary build. Its
  dispersion is session drift with layout held fixed.

## 3. Order

296 executions in thirty-two cycles, `i` from 0 to 31:

    if i mod 4 == 0:  FIX
    ENS(8i), ENS(8i+1), ENS(8i+2), ENS(8i+3)
    CTL(i)
    ENS(8i+4), ENS(8i+5), ENS(8i+6), ENS(8i+7)

The ensemble runs in member-index order, which is the order v1 §5.2 gives the
measured session; the interleaved CTL and FIX groups spread evenly across the
session so drift is measured rather than assumed.

## 4. The statistic

As pilot 1 §6: `s_R(c)` is the sample standard deviation over a group's builds
of the natural logarithm of the build's pooled rate at cell `c`. The half-split
null is the ratio of the pooled rate over even-`j` members to the pooled rate
over odd-`j` members, cell by cell, and the geometric mean of those ratios over
the pinned set. Both estimators are validated against the committed evidence
before this pilot runs: applied to
[`2026-08-20-ensemble-reference-arm.csv`](/dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv)
they reproduce receipt 3's audit `reference_log_sd` and `null_ratio` columns at
all thirty-four cells.

## 5. The decision rule

`σ̂(c)` is receipt 3's audit column, unchanged; `floor(c) = σ̂(c)/2`. `s_C(c)` is
receipt 3's committed candidate arm's dispersion, unchanged.

**ADOPT** requires all six:

- **(a) Coverage, per cell.** `s_R(c) ≥ floor(c)` at every one of the
  thirty-four cells.
- **(b) Coverage, whole set.** The root mean square of `s_R` over the pinned set
  is at least the root mean square of `σ̂`, which is 0.033878.
- **(c) Half-split null.** Inside ±5 % at every cell and inside ±2 % on the
  geometric mean, read two-sided.
- **(d) Projected precision.** With `se(c) = √(s_R(c)² + s_C(c)²) / √256`,
  `ln(1.05) ≥ 3 · se(c)` at every cell, and the same at the set level against
  `ln(1.02)`. The projection is reported a second time with `s_C` scaled by the
  cell's measured ratio of this ensemble's `s_R` to Group CTL's; that second
  reading is recorded, and it does not gate.
- **(e) Drift.** Group FIX's dispersion is at most one quarter of the ensemble's
  `s_R` at `bit_backend/or_inplace/words=1` and at
  `bit_backend/xor_inplace/words=1`.
- **(f) Calibration.** Group CTL's `s_R` at those two cells lies within
  `[0.65, 1.55]` of pilot 1's Group V1 values, 0.049436 and 0.030728.

Anything short of all six is **REJECT**: the measurement is recorded with its
numbers and no amendment is written on this axis set. There is no escalation
rung: the pilot measures the whole ensemble, so a failure is the ensemble's, not
a sample's.

## 6. What the pilot does not decide

- It measures no candidate revision, so it produces no verdict ratio and no
  measurement of v1 §6.4's decorrelation precondition, which needs two arms.
  Decorrelation is decided by the measured session's own audit.
- Its precision reading is a projection from receipt 3's committed candidate
  arm, not a measurement of the amended ensemble's candidate arm. The measured
  session's audit decides precision.
- It does not license a fourth measured session. That session runs under the v2
  amendment after the owner of epic `6dc81018` approves it.

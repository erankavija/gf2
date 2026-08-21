# Predeclared pilot 3: the corrected half-split assignment

Pilot 2, predeclared in
[`ensemble-axes-pilot-2-protocol.md`](ensemble-axes-pilot-2-protocol.md), reads
**REJECT**. Its coverage, precision and drift gates all hold — `s_R` clears
`σ̂`/2 at all thirty-four cells — and two gates fail: the half-split null at four
cells, and the calibration guard.

Both failures are diagnosed from evidence that is not the pilot's own decision
statistic, and this document fixes the pilot that answers them. It is written
before the third pilot's first timed window. Pilots 1 and 2 stand as taken and
are recorded with their numbers; this document revises neither.

τ_cell stays 5 %. τ_set stays 2 %. The pinned set keeps its thirty-four cells.
The schema token is not bumped. The attribution margin stays three standard
errors. `σ̂` and its floors are receipt 3's, unchanged.

## 1. What pilot 2 got wrong, from build-time facts

Pilot 2's enumeration set `G(j) = (E(j) + j) mod 2`. The realized `.text`
address of every one of its 256 members is `base(G) + 32·E`, with
`base(0) = 0x1cbc0` and `base(1) = 0x1f5d0`, so a member's address modulo 64 —
its offset inside a cache line — is `16·G + 32·(E mod 2)` and takes four values.
Under that `G`, **the two halves of the member-index parity split occupy
disjoint sets of them**: the even half holds 64 members at offset 0 and 64 at
48, the odd half 64 at 16 and 64 at 32.

That is the "difference between two unlike halves" v1 §3.2 exists to prevent,
and it is a property of the enumeration, computable from the build log without
any timed measurement. Each marginal was balanced; the joint distribution of
line offset was not.

## 2. The corrected enumeration

Member `j`, for `j` from 0 to 255:

    E(j) = ⌊j / 2⌋                        G(j) = (⌊j / 4⌋ + j) mod 2

| Axis | Option | Levels |
|---|---|---|
| E | `-C link-arg=-Wl,--build-id=0x<hexadecimal string of 2·(20 + 32·E) zeros>` | 0–127 |
| G | `-C link-dead-code` | 0–1 |

A level of 0 omits its option, so member 0 builds under empty `RUSTFLAGS` and is
the ordinary build. Member `j` records `--execution j+1`. **K = 256.**

Verified from the build log's realized addresses, before this pilot runs:

- **256 distinct members** and **256 distinct `.text` page offsets**.
- **The halves are matched on the cache-line offset**: each holds exactly 32
  members at each of the four offsets 0, 16, 32 and 48.
- **The halves are matched on the L1i set index**: the two multisets of
  `(address >> 6) mod 64` are equal, each covering all sixty-four sets twice.
- **The halves are balanced on each axis**: exactly one member at each of the
  128 levels of E, and sixty-four at each of the two levels of G.

The parity property v1 §3.2 states over level sums is not what carries this.
The balance is established directly, on the realized placement, which is the
quantity v1 §3.2's parity property exists to balance and a stronger statement
than it makes.

## 3. What the pilot measures

The 256 binaries are pilot 2's, unchanged and unrebuilt: the set of `(E, G)`
members is the same set, and the correction relabels which member index — and
so which half — each binary carries. Their SHA-256 are recorded in the receipt.

As pilot 2: all 256 members of the reference revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6`, one execution of one repetition
each at `--target-ms 250` over the thirty-four pinned cells, in one lock session
under `dev/scripts/ccx1-bench-flock.sh` with `GF2_BENCH=1`, interleaved with
Group CTL (pilot 1's thirty-two Group V1 binaries) and Group FIX (eight
executions of the ordinary build). The slot order is pilot 2's, unchanged:

    for i from 0 to 31:
        if i mod 4 == 0:  FIX
        ENS(8i) … ENS(8i+3), CTL(i), ENS(8i+4) … ENS(8i+7)

## 4. Robust guards

Pilot 2's calibration guard failed on a single execution. Group CTL's
measurements track pilot 1's Group V1, binary for binary, at every execution but
two: execution 24 recorded 6.534 ns/call against 3.739 at
`bit_backend/or_inplace/words=1` and 4.416 against 3.070 at
`bit_backend/xor_inplace/words=1`, and pilot 1's own execution 1 recorded 4.052
against 3.527. Group FIX carries one of its own: its seventh execution stands
above the other seven at several `simd` cells. These are the process-level
excursions plan §7 and receipt 3 both record; one of them moves a
thirty-two-sample standard deviation by a factor of two.

The guards therefore read robustly, and only the guards:

- **Drift.** `MADN(FIX)`, the median absolute deviation of the group's log rates
  scaled by 1.4826, is at most one quarter of the ensemble's `s_R` at
  `bit_backend/or_inplace/words=1` and at `bit_backend/xor_inplace/words=1`.
- **Calibration.** At each of those two cells, at least thirty of the
  thirty-two Group CTL executions agree within 5 % with pilot 1's Group V1
  measurement of the same binary at the same execution index. Every disagreeing
  execution is named with both numbers.

**The ensemble's own statistic excludes nothing.** No execution of Group ENS is
dropped, trimmed or robustified for any reason. An excursion landing in it is
recorded with its numbers and kept, per plan §7 and v1 §7.3.

## 5. The decision rule

`floor(c) = σ̂(c)/2` from receipt 3's audit, unchanged; `s_C(c)` is receipt 3's
committed candidate arm, unchanged. **ADOPT** requires all six:

- **(a) Coverage, per cell.** `s_R(c) ≥ floor(c)` at all thirty-four cells.
- **(b) Coverage, whole set.** RMS of `s_R` over the pinned set ≥ 0.033878.
- **(c) Half-split null.** Inside ±5 % at every cell and ±2 % on the geometric
  mean, read two-sided.
- **(d) Projected precision.** With `se(c) = √(s_R(c)² + s_C(c)²)/√256`,
  `ln(1.05) ≥ 3·se(c)` at every cell, and the set level likewise against
  `ln(1.02)`. A second reading with `s_C` scaled by the cell's ratio of this
  ensemble's `s_R` to Group CTL's is recorded and does not gate.
- **(e) Drift**, as §4.
- **(f) Calibration**, as §4.

Anything short of all six is **REJECT**: the measurement is recorded with its
numbers and no amendment is written on this axis set. There is no escalation
rung and no further pilot on this enumeration.

## 6. What the pilot does not decide

- It measures no candidate revision, so no verdict ratio and no measurement of
  v1 §6.4's decorrelation precondition, which needs two arms.
- Its precision reading is a projection from receipt 3's committed candidate
  arm, built under different axes. The measured session's audit decides
  precision and decorrelation.
- It does not license a fourth measured session. That session runs under the v2
  amendment after the owner of epic `6dc81018` approves it.

# QC route and compact-lane decision

Decision record for @/issue/1d787ed3. The decision is exploratory: it starts delivery of the QC-aware intra-frame decoder, and the family of @/issue/55c3e433 named `adoption-family` decides adoption. All timings come from one host (AMD Ryzen 9 5900X, 12 cores, 24 logical CPUs) in pilots of twelve pairs per cell. They support no performance claim beyond the rule below.

## Evidence

- Campaign `55c3e433-nr-route-pilot-pilot`, family `55c3e433-nr-route-pilot`, receipt SHA-256 `0a98003957cce247411aa123685f5b8385bf0d2604716f490d324563d9a0891e`, Rust 1.95, 10 sessions, verdict Accepted, no flagged window in any cell (0/120 each).
- Iteration record SHA-256 `67a04756385312a8264f0eadb70769252cc102920f5be863f37d7deb1decd02d`: 176 rows, 132 direct (gf2 single-frame and QC) and 44 replay-derived (AFF3CT).
- The receipt and record are preserved outside the tracked tree.
- Speedup is single-frame time divided by candidate time, from the receipt's paired cells. Intervals are 0.975 bootstrap intervals.

## Rule application

| Clause | Cell | Measured | Threshold | Met |
|---|---|---|---|---|
| Bit and graph checks pass | 8 timed frames | graph equal, posterior bits equal, iteration counts and syndrome flags equal to the canonical decoder | pass | yes |
| QC speedup lower bound | `w1-b1` | 9.0835, lower 9.0657 | at least 1.25 | yes |
| QC speedup lower bound | `p12-b8` | 8.3390, lower 8.3092 | at least 1.25 | yes |
| Indirect AFF3CT time / QC time | `w1-b1` | 9.0835 / 1.7383 = 5.23; conservative 9.0657 / 1.7455 = 5.19 | at least 1/1.1 = 0.909 | yes |
| Indirect AFF3CT time / QC time | `p12-b8` | 8.3390 / 1.5089 = 5.53; conservative 8.3092 / 1.5935 = 5.21 | at least 0.909 | yes |

The AFF3CT speedups over the single-frame arm are 1.7383 (interval 1.7334 to 1.7455) at `w1-b1` and 1.5089 (1.4371 to 1.5935) at `p12-b8`. The indirect ratio divides the two pairings and compares different executables in different pairs; it is indirect, with no direct QC against AFF3CT pairing.

**Decision: go.** All clauses hold at both cells. The other cells agree in direction: QC speedups are 9.0033 (`w1-b8`), 8.8180 (`p6-b8`) and 5.9149 (`l24-b8`, lower bound 5.8446).

## Iteration counts per frame

The timed prefix is eight frames, identical on every worker. Counts per frame:

| Arm | Source | `w1-b1` | Eight-frame cells |
|---|---|---|---|
| gf2 single-frame | direct | 41 | 41, 45, 30, 26, 44, 33, 28, 36 |
| QC | direct | 41 | 41, 45, 30, 26, 44, 33, 28, 36 |
| AFF3CT | replay-derived | 41 | 41, 44, 30, 26, 45, 33, 28, 37 |

gf2 and QC agree on every frame. AFF3CT's counts come from replaying the prefix on the counting prototype after timing, and differ on frames 1, 4 and 7 (45 against 44, 44 against 45, 36 against 37). The totals are 283 for gf2 and QC and 284 for AFF3CT, so the iteration difference does not account for the time ratio.

## Result that would have reversed the decision

A QC speedup lower bound below 1.25 at `w1-b1` or `p12-b8` (observed lower bounds 9.0657 and 8.3092), a posterior-bit mismatch on the timed frames, or an indirect AFF3CT-to-QC time ratio below 0.909. The last needs AFF3CT's speedup over single-frame above 9.99 at `w1-b1` (observed 1.7383) or above 9.17 at `p12-b8` (observed 1.5089), at the observed QC speedups.

## Compact-lane and selective NR lane routing (REQ-04)

**Hold both.** No new work item starts them.

- The pilot has no lane arm: its arms are the forced single-frame route, the QC prototype and AFF3CT. It measured no lane route.
- The QC go serves the NR single-frame cell (`w1-b1`) and the twelve-worker cell (`p12-b8`) under `qc-go-rule`. Selective NR lanes have no remaining cell to serve, and the QC speedups at `w1-b8`, `p6-b8` and `l24-b8` exceed the single-frame arm by 5.9 or more.
- DVB-T2 lanes are unmeasured in this family. Their result is recorded by the closeout, `gap-closeout`.

A held candidate restarts from a measured lane arm at an NR cell where QC loses or a DVB-T2 cell where lanes lead, not from this pilot.

## Production behavior and the numerical contract (REQ-05)

- `git diff 3ea453e42 HEAD -- crates/gf2-coding/src/ldpc` is empty: no production decoder source changed.
- `crates/gf2-coding/src/tuning.rs` differs by one doc-comment line, which names the coding producer's new location. No code changed.
- The gf2 single-frame arm runs the production decoder. The QC arm matches the canonical decoder's posterior bits, iteration counts and syndrome flags on the eight timed frames. Both stay within `docs/reference/ldpc-decoder-numerical-contract.md`; the QC decoder's conformance suites belong to its delivery.
- AFF3CT is outside the contract: it adds the channel LLR after summing check messages, where the contract sums from the channel LLR in edge order; it takes sign bits, where the contract takes the comparison $\mu \ge 0$ and reads signed zero as positive and NaN as negative; and it skips the terminal syndrome check at the iteration cap. Its decisions can therefore differ, as in frames 1, 4 and 7 above.

## Build note

The AFF3CT arm was built against a host-local checkout of AFF3CT, with GCC 16.2.1, `-O3 -march=native`. A fresh clone cannot rebuild it without that checkout.

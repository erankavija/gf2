# QC route and compact-lane decision

Decision record for @/issue/1d787ed3. The decision is exploratory: it starts delivery of the QC-aware intra-frame decoder, and the family of @/issue/55c3e433 named `adoption-family` decides adoption. All timings come from one host (AMD Ryzen 9 5900X, 12 cores, 24 logical CPUs) in pilots of twelve pairs per cell. They support no performance claim beyond the rule below.

## Evidence

- Campaign `55c3e433-nr-route-pilot-pilot`, family `55c3e433-nr-route-pilot`, receipt SHA-256 `0a98003957cce247411aa123685f5b8385bf0d2604716f490d324563d9a0891e`, Rust 1.95, 10 sessions, verdict Accepted, no flagged window in any cell (0/120 each).
- Iteration record SHA-256 `67a04756385312a8264f0eadb70769252cc102920f5be863f37d7deb1decd02d`: 176 rows, 132 direct (gf2 single-frame and QC) and 44 replay-derived (AFF3CT).
- The receipt, iteration record and check output are preserved outside the tracked tree.
- Speedup is single-frame time divided by candidate time, from the receipt's paired cells. Intervals are 0.975 bootstrap intervals.

## Rule application

On the 5900X host, all three clauses of `qc-go-rule` hold, so the decision is **go**.

The bit and graph clause holds. The parity-check matrices are equal, and on the eight timed frames every posterior word's `f32` bit pattern equals the canonical decoder's, as do hard decisions, iteration counts and syndrome flags. The comparison covers 208,896 posterior words, and its output has SHA-256 `2d10ad214cb7ca8f1d15692e9fcc2094aba23a93d7472e9c2449fc4c1ccfe25d`. The check that ran before timing compared hard decisions, iteration counts and syndrome flags only. The bit-pattern comparison ran after timing, against the same prototype source and inputs.

The speedup clause holds: QC's lower bound over the single-frame arm is 9.0657 at `w1-b1` and 8.3092 at `p12-b8`, against a threshold of 1.25.

The indirect clause holds: AFF3CT time over QC time is 5.23 at `w1-b1` and 5.53 at `p12-b8`, against a threshold of $1/1.1$. Dividing QC's lower bound by AFF3CT's upper bound gives 5.19 and 5.21. The ratio divides two pairings, so it compares executables that were never paired directly.

## Iteration counts per frame

The timed prefix is eight frames, identical on every worker, and the iteration record lists each arm's count per frame. gf2 and QC agree on every frame. AFF3CT's counts are replay-derived and differ by one iteration on three of the eight frames, with totals of 284 against 283, so the iteration difference does not account for the time ratio.

## Result that would have reversed the decision

A QC speedup lower bound below 1.25 at `w1-b1` or `p12-b8` (observed lower bounds 9.0657 and 8.3092), a posterior-bit mismatch on the timed frames, or an indirect AFF3CT-to-QC time ratio below 0.909. The last needs AFF3CT's speedup over single-frame above 9.99 at `w1-b1` (observed 1.7383) or above 9.17 at `p12-b8` (observed 1.5089), at the observed QC speedups.

## Compact-lane and selective NR lane routing (REQ-04)

**Hold both.** No new work item starts them.

- The pilot has no lane arm: its arms are the forced single-frame route, the QC prototype and AFF3CT. It measured no lane route.
- The `qc-go-rule` predicate routes `w1-b1` and `p12-b8` to QC, and every other case keeps its existing route. Whether selective NR lanes would help at `p6-b8`, `l24-b8` or elsewhere stays unmeasured, because no lane arm ran. Delivering the QC route comes first, since it changes the baseline such an experiment compares with.
- DVB-T2 lanes are unmeasured in this family. Their result is recorded by the closeout, `gap-closeout`.

A held candidate restarts from a measured lane arm at an NR cell where QC loses or a DVB-T2 cell where lanes lead, not from this pilot.

## Production behavior and the numerical contract (REQ-05)

- `git diff 3ea453e42 HEAD -- crates/gf2-coding/src/ldpc` is empty: no production decoder source changed.
- `crates/gf2-coding/src/tuning.rs` differs by one doc-comment line, which names the coding producer's new location. No code changed.
- The gf2 single-frame arm runs the production decoder. The QC arm matches the canonical decoder's posterior bits, iteration counts and syndrome flags on the eight timed frames. Both stay within `docs/reference/ldpc-decoder-numerical-contract.md`; the QC decoder's conformance suites belong to its delivery.
- AFF3CT is outside the contract: it adds the channel LLR after summing check messages, where the contract sums from the channel LLR in edge order; it takes sign bits, where the contract takes the comparison $\mu \ge 0$ and reads signed zero as positive and NaN as negative; and it skips the terminal syndrome check at the iteration cap. These differences can change iteration counts and decisions; the pilot's record shows differing iteration counts only.

## Build note

The AFF3CT arm was built against a host-local checkout of AFF3CT, with GCC 16.2.1, `-O3 -march=native`. A fresh clone cannot rebuild it without that checkout.

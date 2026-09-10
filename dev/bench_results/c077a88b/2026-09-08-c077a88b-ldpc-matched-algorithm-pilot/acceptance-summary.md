# Acceptance summary for `c077a88b-2026-09-08-matched-algorithm-pilot`

Label `Pilot`; verdict **Rejected**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `891ef421ee18c3c7cbdb26ebb6a892505df68a03a0b6b318ef9bfcbc8056df22`.

Family `ldpc-matched-algorithm-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-matched-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | n/a | n/a | n/a | **Invalid** |  |
| `nr-bg1-z384-matched-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | n/a | n/a | n/a | **Invalid** |  |

## Findings

- P-18 Error `dvb-t2-r12-matched-pilot`: decoder cell reports no BER/FER evidence
- P-18 Error `nr-bg1-z384-matched-pilot`: decoder cell reports no BER/FER evidence

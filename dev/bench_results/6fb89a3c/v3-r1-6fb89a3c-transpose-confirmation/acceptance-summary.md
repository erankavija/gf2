# Acceptance summary for `6fb89a3c-v3-r1-transpose-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `b76650da89dc9c611bba91e34524157e3f8e844437282d838de90875f2c5e80c`.

Family `transpose-vs-external`: 8 comparisons at family-wise alpha 0.025, per-comparison confidence 0.996875, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `transpose-64-canonical-vs-m4ri` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3222 | [0.3202, 0.3237] at 0.9969 | Regressed | **NotConfirmatory** |  |
| `transpose-64-canonical-vs-bitshuffle` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1903 | [0.1895, 0.2005] at 0.9969 | Regressed | **NotConfirmatory** |  |
| `transpose-consumer-63-vs-m4ri` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0623 | [1.0560, 1.0739] at 0.9969 | NotWorse | **NotConfirmatory** |  |
| `transpose-consumer-64-vs-m4ri` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7944 | [0.7885, 0.8038] at 0.9969 | Regressed | **NotConfirmatory** |  |
| `transpose-consumer-65-vs-m4ri` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.8725 | [1.8523, 1.9000] at 0.9969 | Improved | **NotConfirmatory** |  |
| `transpose-consumer-64-vs-bitshuffle-adapter` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4928 | [0.4866, 0.4996] at 0.9969 | Regressed | **NotConfirmatory** |  |
| `transpose-consumer-63-vs-bitshuffle-adapter` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.2313 | [0.2290, 0.2340] at 0.9969 | Regressed | **NotConfirmatory** |  |
| `transpose-consumer-65-vs-bitshuffle-adapter` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4964 | [0.4919, 0.5007] at 0.9969 | Regressed | **NotConfirmatory** |  |

## Findings

- P-20 Note `transpose-64-canonical-vs-m4ri`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-64-canonical-vs-bitshuffle`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-consumer-63-vs-m4ri`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-consumer-64-vs-m4ri`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-consumer-65-vs-m4ri`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-consumer-64-vs-bitshuffle-adapter`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-consumer-63-vs-bitshuffle-adapter`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `transpose-consumer-65-vs-bitshuffle-adapter`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates

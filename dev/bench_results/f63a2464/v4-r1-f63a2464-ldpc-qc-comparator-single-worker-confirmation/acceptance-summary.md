# Acceptance summary for `f63a2464-v4-r1-qc-comparator-single-worker-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `85fd7af221e8b5c05422ceb105c68dd1ecea898f6e6a35aa7cd188404faf8a0c`.

Family `ldpc-qc-comparator-single-worker-v1`: 2 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.0125 per comparison (confidence 0.987500), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg1-z384-qc-comparator-w1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1937 | [0.1920, 0.1948] at 0.9875 | Regressed | **Fail** |  |
| `nr-bg1-z384-qc-comparator-latency` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1934 | [0.1926, 0.1940] at 0.9875 | Regressed | **Fail** |  |

## Findings

No findings.

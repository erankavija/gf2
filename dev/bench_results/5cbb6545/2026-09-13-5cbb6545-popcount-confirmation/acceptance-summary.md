# Acceptance summary for `popcount-confirmation-5cbb6545-20260913t132314z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `6ef6c7d0a95a0aedd6b3616401e11b2eef11ab39c349f29c3fc6daa9a6aac335`.

Family `popcount-route-selection`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-w4-dispatch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9054 | [0.9030, 0.9187] at 0.9958 | Regressed | **Fail** |  |
| `popcount-w8-dispatch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1085 | [1.1032, 1.1252] at 0.9958 | Improved | **Pass** |  |
| `popcount-w128-dispatch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0432 | [1.0392, 1.0463] at 0.9958 | NotWorse | **Pass** |  |
| `popcount-w1024-dispatch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3449 | [1.3437, 1.3480] at 0.9958 | Improved | **Pass** |  |
| `popcount-w16384-dispatch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3438 | [1.3399, 1.3461] at 0.9958 | Improved | **Pass** |  |
| `popcount-w16384-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0292 | [1.0263, 1.0327] at 0.9958 | NotWorse | **NotMaterial** |  |

## Findings

No findings.

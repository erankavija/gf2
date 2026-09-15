# Acceptance summary for `confirmation-polynomial-53c5a8c0-20260913t131633z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `00416b2834dd671d50e7915fea4da11b66498b11252e243d899249e93b669dcf`.

Family `wide-polynomial-competitiveness`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `wide-field-256-composed` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3917 | [1.3850, 1.4142] at 0.9958 | Improved | **Pass** |  |
| `wide-field-571-composed` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1148 | [1.1123, 1.1216] at 0.9958 | NotWorse | **NotMaterial** |  |
| `poly-4w` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8282 | [0.7998, 0.8377] at 0.9958 | Regressed | **Fail** |  |
| `poly-9w` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7673 | [0.7653, 0.7689] at 0.9958 | Regressed | **Fail** |  |
| `poly-16w` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 97.0714 | [96.6891, 97.1873] at 0.9958 | Improved | **Pass** |  |
| `raw-batch-1024-composed` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0809 | [0.0808, 0.0810] at 0.9958 | Regressed | **Fail** |  |

## Findings

No findings.

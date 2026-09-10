# Acceptance summary for `c7113c5a-v3-r1-host-targeting-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `fe25d2360371af54e9f6914613408a851dba55dcf292534eaadfaf0d5125409c`.

Family `polynomial-host-targeting`: 4 comparisons at family-wise alpha 0.025, per-comparison confidence 0.993750, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `poly-mul-4w-conservative-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6886 | [0.6867, 0.6907] at 0.9938 | Regressed | **Fail** |  |
| `poly-mul-4w-tuned-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6304 | [0.6291, 0.6325] at 0.9938 | Regressed | **Fail** |  |
| `poly-mul-256w-conservative-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 288.3635 | [287.5569, 289.7804] at 0.9938 | Improved | **Pass** |  |
| `poly-mul-256w-tuned-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 304.3475 | [303.7934, 305.2465] at 0.9938 | Improved | **Pass** |  |

## Findings

No findings.

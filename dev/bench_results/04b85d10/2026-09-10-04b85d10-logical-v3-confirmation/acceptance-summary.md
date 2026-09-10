# Acceptance summary for `confirmation-v3-logical-04b85d10-20260910t162730z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `bade034cd22dfe569ac7a711fc14d3539cb814c1fcf360dfe9f6aee076eb85d9`.

Family `bit-storage-logical-consumers`: 4 comparisons at family-wise alpha 0.025, per-comparison confidence 0.993750, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `logical-row-xor-dispatch-64w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0570 | [1.0510, 1.0975] at 0.9938 | NotWorse | **NotMaterial** |  |
| `logical-row-xor-dispatch-8w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1161 | [1.1082, 1.1218] at 0.9938 | Improved | **Pass** |  |
| `logical-row-xor-threshold-4w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3532 | [1.3477, 1.3626] at 0.9938 | Improved | **Pass** |  |
| `logical-row-xor-dispatch-8192w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9956 | [0.9426, 1.0353] at 0.9938 | NotWorse | **NotMaterial** |  |

## Findings

No findings.

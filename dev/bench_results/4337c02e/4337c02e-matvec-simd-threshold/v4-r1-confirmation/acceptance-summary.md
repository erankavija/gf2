# Acceptance summary for `v4-r1-confirmation-4337c02e-matvec-simd-threshold`

Label `Confirmation`; verdict **Rejected**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `d9c3919d493f8433f88227b9778e1279615a355fcefbb0bee41789c0f1a8cce9`.

Family `4337c02e-matvec-simd-threshold`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-r1024-8w-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0682 | [1.0635, 1.0739] at 0.9958 | NotWorse | **NotMaterial** |  |
| `lane-r1024-9w-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3449 | [1.3391, 1.3472] at 0.9958 | Improved | **Pass** |  |
| `lane-r1024-32w-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8042 | [0.8000, 0.8079] at 0.9958 | Regressed | **Fail** |  |
| `lane-r1024-63w-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1622 | [1.0948, 1.1944] at 0.9958 | Improved | **Pass** |  |
| `lane-r1024-64w-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0017 | [0.9825, 1.0399] at 0.9958 | NotWorse | **NotMaterial** |  |
| `lane-r1024-65w-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0678 | [1.0504, 1.1068] at 0.9958 | NotWorse | **NotMaterial** |  |

## Findings

- P-03 Error: declared measurement resolution is below the verified pilot half-width 0.22779674714617634

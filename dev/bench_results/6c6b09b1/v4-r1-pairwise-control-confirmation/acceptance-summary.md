# Acceptance summary for `6c6b09b1-v4-r1-pairwise-control-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `049706676a5a527b968aa4ca7c60e5d2817bc5441d5f338b6be3c6fb15250612`.

Family `byte-field-pairwise-control`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `pairwise-4k-batch-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1301 | [1.1011, 1.1670] at 0.9958 | NotWorse | **NotMaterial** |  |
| `pairwise-128k-batch-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8298 | [0.8163, 0.8520] at 0.9958 | Regressed | **Fail** |  |
| `pairwise-128k-whole-batch-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0889 | [1.0778, 1.1068] at 0.9958 | NotWorse | **NotMaterial** |  |
| `pairwise-4k-batch-vs-gfcomplete` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5390 | [0.5365, 0.5405] at 0.9958 | Regressed | **Fail** |  |
| `pairwise-128k-batch-vs-gfcomplete` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5313 | [0.5306, 0.5356] at 0.9958 | Regressed | **Fail** |  |
| `pairwise-128k-whole-batch-vs-gfcomplete` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6883 | [0.6876, 0.7033] at 0.9958 | Regressed | **Fail** |  |

## Findings

No findings.

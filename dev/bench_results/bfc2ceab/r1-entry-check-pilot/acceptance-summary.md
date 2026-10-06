# Acceptance summary for `bfc2ceab-r1-entry-check-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `17bf9e14205bd5684c64285dfe4e5d4b500f7f7074a1ed1c4e175a429fa2b06f`.

Family `caller-trusted-entry-check-cost`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `spmm-small-n16` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9327 | [0.9309, 0.9349] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-small-n1024` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9937 | [0.9931, 0.9944] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-medium-n16` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9373 | [0.9345, 0.9405] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-medium-n1024` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9911 | [0.9903, 0.9922] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `permanent-f3-n14` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9972 | [0.9780, 1.0028] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `ple-small-n128` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0018 | [0.9986, 1.0050] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `ple-medium-n128` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9932 | [0.9917, 1.0000] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `gemm-small-n32` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9851 | [0.9797, 0.9893] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `gemm-medium-n32` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0010 | [1.0000, 1.0025] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

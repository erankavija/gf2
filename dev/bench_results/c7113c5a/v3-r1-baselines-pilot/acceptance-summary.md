# Acceptance summary for `c7113c5a-v3-r1-baselines-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `f8d260031b5357a40bac00e324fc875f8a6bbec72d6c248188fe17ab43c8aae1`.

Family `polynomial-multiplication-baselines`: 10 comparisons at family-wise alpha 0.025, per-comparison confidence 0.997500, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `poly-mul-4w-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6129 | [0.6087, 0.6190] at 0.9975 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-9w-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6530 | [0.6438, 0.6627] at 0.9975 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-64w-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 166.9638 | [164.6444, 167.8100] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 299.3417 | [289.3470, 301.2773] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-2048w-streaming-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 865.5127 | [863.8984, 866.5525] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 164.2995 | [157.9586, 183.1765] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-12core` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 12 | 0/120 | 157.3670 | [122.6601, 194.7959] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-24smt` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 12 | 0/120 | 169.0316 | [163.0292, 173.9969] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-4w-public-api-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 45.8422 | [45.7282, 46.0351] at 0.9975 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `internal-clmul-batch-1024-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.1535 | [0.1518, 0.1564] at 0.9975 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `internal-gf2m-dot-1024-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.1561 | [0.1516, 0.1599] at 0.9975 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

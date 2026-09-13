# Acceptance summary for `selected-1d4fd63d-20260913t122340z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `e179554ac444507e56ec8b9ff4c4db461b9bb9e6cf531984c770e69a32463bac`.

Family `transpose-lane-selection`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-avx2-ymm6-block-256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.2425 | [1.2259, 1.2608] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-bulk-4096-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 1.2173 | [1.2074, 1.2329] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-bitslice-absorb-m14-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0237 | [1.0217, 1.0255] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-bitslice-unpack-m14-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.2007 | [1.1747, 1.2760] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-matrix-transpose-4096-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0888 | [1.0824, 1.0954] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-matrix-transpose-65-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1142 | [1.0957, 1.1258] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-encode-bitslice-m14-b256-control-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9997 | [0.9988, 1.0004] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

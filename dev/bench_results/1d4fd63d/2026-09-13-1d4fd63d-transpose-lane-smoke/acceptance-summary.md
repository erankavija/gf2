# Acceptance summary for `smoke-1d4fd63d-20260913t120848z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `a13a708614e59d0a859d6b9c7ee81b11dc5d9042b2d4a90561b33105572a9873`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `transpose-lane-smoke`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-avx2-ymm6-smoke-block-16-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1963 | [1.1876, 1.2421] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-smoke-bulk-64-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 0.0049 | [0.0049, 0.0049] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-smoke-absorb-m8-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9052 | [0.8977, 0.9148] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-smoke-unpack-m8-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2769 | [1.1865, 1.3219] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-smoke-matrix-128-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8968 | [0.8808, 0.9223] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-encode-bitslice-m8-b8-smoke-control-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0039 | [0.9987, 1.0073] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

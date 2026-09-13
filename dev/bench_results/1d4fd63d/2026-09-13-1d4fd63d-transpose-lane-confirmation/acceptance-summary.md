# Acceptance summary for `confirmation-1d4fd63d-20260913t122951z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `34f0ab4be8a3f9ffcbb453a0833cb1aa258bac384fc0df394086894dc82bf414`.

Family `transpose-lane-selection`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-avx2-ymm6-block-256-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2456 | [1.2330, 1.2553] at 0.9958 | Improved | **Pass** |  |
| `lane-avx2-ymm6-bulk-4096-6core` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 1.2239 | [1.2066, 1.2429] at 0.9958 | Improved | **Pass** |  |
| `lane-avx2-ymm6-bitslice-absorb-m14-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0226 | [1.0192, 1.0239] at 0.9958 | NotWorse | **NotMaterial** |  |
| `lane-avx2-ymm6-bitslice-unpack-m14-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1970 | [1.1827, 1.2687] at 0.9958 | Improved | **Pass** |  |
| `lane-avx2-ymm6-matrix-transpose-4096-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0905 | [1.0857, 1.0959] at 0.9958 | NotWorse | **NotMaterial** |  |
| `lane-avx2-ymm6-matrix-transpose-65-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1140 | [1.0947, 1.1524] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.

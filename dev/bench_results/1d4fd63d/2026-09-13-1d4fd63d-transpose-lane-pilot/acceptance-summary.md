# Acceptance summary for `pilot-1d4fd63d-20260913t121257z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `62c4e70acf32fabc38213e41926677c95d567b57c0345f3f19196362cb891a1f`.

Family `transpose-lane-selection`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-avx2-ymm6-block-256-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2416 | [1.2302, 1.2544] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-bulk-4096-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 1.2349 | [1.2164, 1.2441] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-bitslice-absorb-m14-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0213 | [1.0184, 1.0244] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-bitslice-unpack-m14-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2730 | [1.1809, 1.3026] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-matrix-transpose-4096-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0885 | [1.0859, 1.0976] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-matrix-transpose-65-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1154 | [1.0918, 1.1439] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-block-256-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0050 | [0.0049, 0.0050] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-bulk-4096-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 0.0050 | [0.0050, 0.0051] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-bitslice-absorb-m14-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.1594 | [0.1586, 0.1604] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-bitslice-unpack-m14-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0339 | [0.0336, 0.0348] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-matrix-transpose-4096-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0173 | [0.0172, 0.0174] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-matrix-transpose-65-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0780 | [0.0769, 0.0809] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-block-256-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.7584 | [0.7275, 0.7611] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-bulk-4096-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 0.7332 | [0.7289, 0.7603] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-bitslice-absorb-m14-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0036 | [1.0003, 1.0067] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-bitslice-unpack-m14-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8920 | [0.8569, 0.9402] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-matrix-transpose-4096-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9577 | [0.9497, 0.9614] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-matrix-transpose-65-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8787 | [0.8697, 0.9245] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-encode-bitslice-m14-b256-control-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9996 | [0.9991, 1.0005] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

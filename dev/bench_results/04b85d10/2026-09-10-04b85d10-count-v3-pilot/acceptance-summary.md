# Acceptance summary for `pilot-v3-count-04b85d10-20260910t161440z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `e0d76094b61fde69d7e2ddf2bd5b1e37568935925c54d004c97eaf7278be06bb`.

Family `bit-storage-count-consumers`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `count-popcount-dispatch-4w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.5445 | [1.5404, 1.5473] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `count-popcount-threshold-8w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.2010 | [1.1986, 1.2017] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `count-popcount-bandwidth-65536w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 3.1937 | [3.1732, 3.2126] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `count-zero-test-507w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1187 | [1.1176, 1.1194] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `count-ldpc-check-64800-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0018 | [1.0000, 1.0044] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `count-dense-matvec-1024x4096-control-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0014 | [0.9832, 1.0223] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

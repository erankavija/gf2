# Acceptance summary for `pilot-v3-logical-04b85d10-20260910t161146z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `fe1c361bb4cb94a763c2f98acc3f9b1b5decea163d06106d730419770f279492`.

Family `bit-storage-logical-consumers`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `logical-row-xor-dispatch-64w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0672 | [1.0553, 1.1081] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `logical-row-xor-dispatch-8w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1128 | [1.1067, 1.1384] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `logical-row-xor-threshold-4w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.3578 | [1.3469, 1.3708] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `logical-row-xor-dispatch-8192w-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9934 | [0.9362, 1.0581] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `logical-dense-rref-1024-control-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0004 | [0.9971, 1.0060] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `logical-ldpc-syndrome-64800-control-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0025 | [0.9993, 1.0072] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

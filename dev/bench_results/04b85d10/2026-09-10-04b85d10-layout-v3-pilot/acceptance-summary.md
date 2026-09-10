# Acceptance summary for `pilot-v3-layout-04b85d10-20260910t161732z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `8ed58e057d981184c1144a16f76aeeedebfda8c46d8265c36f05096dfaf4a891`.

Family `bit-storage-layout-consumers`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `layout-transpose-block-256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.9229 | [1.8842, 1.9416] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-transpose-block-4096-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 1.9148 | [1.8826, 1.9520] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-bitslice-m14-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.4766 | [2.4742, 2.4791] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-fold-m14-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.6939 | [2.6927, 2.6952] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-caller-buffer-m14-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0030 | [1.0018, 1.0038] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-dense-transpose-4096-control-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9993 | [0.9961, 1.0061] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-dvb-bch-encode-7200-control-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0015 | [0.9978, 1.0033] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

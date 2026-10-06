# Acceptance summary for `v4-r1-4337c02e-matvec-simd-threshold`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 9; resumed true.
Receipt digest `f054c74c060db141226a90ad4332dd167464c6432f404c424f5f9ef44ebbc790`.

Family `4337c02e-matvec-simd-threshold`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-r1024-8w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0676 | [1.0652, 1.0697] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-9w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.3443 | [1.3406, 1.3467] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-32w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.7995 | [0.7906, 0.8041] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-63w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1538 | [1.1400, 1.1611] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-64w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0138 | [0.9913, 1.0242] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-65w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0485 | [1.0349, 1.0583] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-7w-control-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0014 | [0.9993, 1.0032] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-10w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.5855 | [1.5780, 1.5913] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-62w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0964 | [1.0841, 1.1099] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-66w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1370 | [1.1186, 1.1461] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-8w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0489 | [1.0472, 1.0510] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-9w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0759 | [1.0748, 1.0772] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-63w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0671 | [1.0504, 1.0848] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-64w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0520 | [1.0391, 1.0537] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-65w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0517 | [1.0488, 1.0547] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-8w-cold` | Exploratory | SingleCore | 0 | 24 | 1/240 | 1.1185 | [1.0428, 1.3733] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-r1024-64w-cold` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.2630 | [0.9927, 1.3993] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

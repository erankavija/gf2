# Acceptance summary for `v4-r1-2037941f-dense-allocated-matvec`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 12; resumed true.
Receipt digest `7c77f90fb8d345102150ee7ca154b322a2245c9ac73bc70365dffa93da8ee21c`.

Family `2037941f-dense-allocated-matvec`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matvec-r1024-8w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0000 | [0.9956, 1.0020] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-9w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9999 | [0.9981, 1.0011] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-63w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9998 | [0.9811, 1.0256] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-64w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9989 | [0.9632, 1.0062] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-65w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0008 | [0.9970, 1.0059] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-7w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0011 | [0.9994, 1.0026] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-66w-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0001 | [0.9963, 1.0054] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-8w-tail1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0037 | [0.9931, 1.0176] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-9w-tail1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9997 | [0.9974, 1.0023] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-63w-tail1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0022 | [0.9841, 1.0282] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-64w-tail1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0065 | [0.9885, 1.0400] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-65w-tail1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9983 | [0.9699, 1.0069] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-8w-cold` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.2728 | [0.7295, 1.4695] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-64w-cold` | Exploratory | SingleCore | 0 | 24 | 1/240 | 1.0421 | [0.8228, 1.2490] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-8w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9999 | [0.9989, 1.0013] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-9w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9993 | [0.9989, 1.0002] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-63w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9992 | [0.9971, 1.0002] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-64w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9999 | [0.9990, 1.0008] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-65w-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0008 | [0.9993, 1.0017] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-8w-scalar-reference-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.8769 | [0.8733, 0.8806] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-9w-scalar-reference-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.7549 | [0.7534, 0.7577] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-63w-scalar-reference-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.8791 | [0.8732, 0.8842] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-64w-scalar-reference-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9848 | [0.9652, 0.9953] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-65w-scalar-reference-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9407 | [0.9293, 0.9631] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

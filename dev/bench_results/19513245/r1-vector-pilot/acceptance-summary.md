# Acceptance summary for `19513245-r1-vector-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `f472c7f48cef6c743e8c29ccbc29460f704869c274572f1908ed5909b70cddcb`.

Family `bytefield-consumer-vector`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-4k-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.4287 | [2.4141, 2.4404] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.4287 | [2.4239, 2.4355] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-8m-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.1886 | [2.1825, 2.1940] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-2m-stream-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.4480 | [2.4418, 2.4561] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `region-4k-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 51.2294 | [51.0763, 51.4146] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `region-128k-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 58.6222 | [58.4770, 58.6783] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 102.2763 | [100.9550, 103.4006] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 108.2738 | [106.9067, 111.3792] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-8m-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 81.2956 | [79.8457, 82.0114] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-2m-stream-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 79.0505 | [78.8987, 81.2936] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `region-4k-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 141.4122 | [141.1398, 141.6817] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `region-128k-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 162.1598 | [161.6206, 162.3503] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

# Acceptance summary for `d06a99ed-worker-routing-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `75005fda12d792fe068c75d96c7e93ea03a8d787c4c37258f4529a4d06b4d660`.

Family `d06a99ed-worker-aware-production-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-production-w1-b1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9795 | [0.9776, 0.9834] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-production-w1-b8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0026 | [0.9969, 1.0165] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-production-w1-b1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9872 | [0.9862, 0.9895] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-production-w1-b8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9908 | [0.9881, 0.9967] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-production-w12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 1.7629 | [1.7307, 1.7759] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-production-w24-b8` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 2.8357 | [2.8205, 2.8556] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

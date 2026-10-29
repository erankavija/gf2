# Acceptance summary for `31727c57-decoder-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 10; resumed true.
Receipt digest `7d1805834941715300af93fd88b9c935f7be82a7a80acbad2fcff6f802ab2c0b`.

Family `1362381c-decoder-route-integration`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-integrated-w1-b1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0058 | [1.0040, 1.0064] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-integrated-w1-b8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 5.4027 | [5.3929, 5.4227] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-integrated-w6-b8` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 1.0088 | [1.0036, 1.0099] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-integrated-w12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 0.9991 | [0.9866, 1.0073] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-integrated-w24-b8` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 0.9930 | [0.9848, 1.0151] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w1-b1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0073 | [1.0055, 1.0103] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w1-b8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 5.8012 | [5.7803, 5.8133] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w6-b8` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 1.0035 | [1.0001, 1.0113] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 1.0052 | [0.9967, 1.0111] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w24-b8` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 1.0093 | [0.9942, 1.0620] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

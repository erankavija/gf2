# Acceptance summary for `1358d6ab-ldpc-aff3ct-integration-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `b3172409dfdbbaa820f677444358c4d6fcc59f27398250c68de496e57e5a9b48`.

Family `1362381c-ldpc-aff3ct-integration`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-integrated-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3753 | [0.3683, 0.3763] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-integrated-w1-b1` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.9196 | [1.9062, 1.9793] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-r12-integrated-p12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 12 | 0/120 | 1.8156 | [1.7889, 1.8250] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3243 | [0.3117, 0.3255] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-w1-b1` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.7825 | [1.7749, 1.7884] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-z384-integrated-p12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 12 | 0/120 | 1.5461 | [1.5301, 1.6020] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

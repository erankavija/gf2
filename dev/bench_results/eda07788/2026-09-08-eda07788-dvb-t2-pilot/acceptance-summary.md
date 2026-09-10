# Acceptance summary for `pilot-eda07788-20260908t093021z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `ca889363f8180e86cf635f720014c9b373421b12e53b3484825ebc1cbfeaa33f`.

Family `dvb-t2-bit-interleave-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0023 | [0.9900, 1.0176] at 0.9500 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4562 | [0.4522, 0.4628] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-gap-portable-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4550 | [0.4529, 0.4575] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0014 | [0.9875, 1.0365] at 0.9500 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4295 | [0.4260, 0.4326] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-gap-portable-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4276 | [0.4264, 0.4291] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9926 | [0.9900, 0.9971] at 0.9500 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-short-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 3/60 | 0.1673 | [0.1556, 0.1740] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-short-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.1601 | [0.1593, 0.1670] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-streaming-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5269 | [0.5238, 0.5281] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

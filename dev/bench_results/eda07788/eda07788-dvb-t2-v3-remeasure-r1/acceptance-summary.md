# Acceptance summary for `remeasure-v3-eda07788-r1`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 5; resumed true.
Receipt digest `54455f903a55a2264154a48a7d7dfcaed5fa0b769580983a6d82c594e4475760`.

Family `dvb-t2-bit-interleave-baselines`: 12 comparisons at family-wise alpha 0.008333333333333333, per-comparison confidence 0.999306, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-null-native-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9997 | [0.9974, 1.0007] at 0.9993 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4556 | [0.4548, 0.4570] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-gap-portable-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4523 | [0.4516, 0.4534] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-control-portable-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9930 | [0.9909, 0.9953] at 0.9993 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4274 | [0.4267, 0.4282] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-gap-portable-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4274 | [0.4264, 0.4284] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-control-portable-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0008 | [0.9984, 1.0029] at 0.9993 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-short-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1906 | [0.1899, 0.1920] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-short-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1848 | [0.1836, 0.1855] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-streaming-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.5213 | [0.5196, 0.5224] at 0.9993 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

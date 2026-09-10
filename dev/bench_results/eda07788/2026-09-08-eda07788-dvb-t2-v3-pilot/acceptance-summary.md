# Acceptance summary for `pilot-v3-eda07788-20260908t192701z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `d4d178248321fd148a4bb6966f84b7ae27d5967fc77ae061dc4145c7c7f28db5`.

Family `dvb-t2-bit-interleave-baselines`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9996 | [0.9905, 1.0030] at 0.9958 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4883 | [0.4833, 0.4907] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-gap-portable-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4901 | [0.4889, 0.4911] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0006 | [0.9970, 1.0077] at 0.9958 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4576 | [0.4548, 0.4611] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-gap-portable-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4583 | [0.4573, 0.4604] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9998 | [0.9955, 1.0067] at 0.9958 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-short-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.2242 | [0.1898, 0.2528] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-short-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.2215 | [0.2182, 0.2256] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-streaming-gap-native-vs-xdsopl` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5638 | [0.5618, 0.5653] at 0.9958 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

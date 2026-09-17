# Acceptance summary for `v4-r2-9fb40c83-dvb-interleave-profile`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 5; resumed true.
Receipt digest `be4d9ad84207513d80a6c4e9b5d0b2b47ed106b6d73adbe5f8aa1100762833ef`.

Family `dvb-t2-interleave-consumer-profile`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-warm-isolated-null` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9996 | [0.9940, 1.0004] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-warm-sim-stage-gap` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4302 | [0.4269, 0.4311] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-warm-isolated-null` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0001 | [0.9927, 1.0019] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-normal-warm-sim-stage-gap` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4291 | [0.4271, 0.4326] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-short-warm-isolated-null` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0004 | [0.9765, 1.0460] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-short-warm-sim-stage-gap` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.3487 | [0.3482, 0.3496] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-short-warm-isolated-null` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9996 | [0.9794, 1.0042] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam64-r12-short-warm-sim-stage-gap` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.3936 | [0.3897, 0.3938] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-streaming-isolated-null` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0000 | [0.9990, 1.0027] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `dvb-t2-qam16-r12-normal-streaming-sim-stage-gap` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4949 | [0.4937, 0.4957] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

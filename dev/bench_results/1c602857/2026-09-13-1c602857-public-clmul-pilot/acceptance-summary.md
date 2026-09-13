# Acceptance summary for `pilot-1c602857-20260913t010028z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `095e22ee8d767e94118ec8b8c4058ca4aa510a29790c1b7920099d464cf8d743`.

Family `public-clmul-wide-dispatch`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `clmul-wide-4w-owned` | Exploratory | SingleCore | 0 | 6 | 0/60 | 77.3770 | [75.9929, 77.6318] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-9w-owned` | Exploratory | SingleCore | 0 | 6 | 0/60 | 114.0407 | [113.9386, 114.2861] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-4w-accumulate` | Exploratory | SingleCore | 0 | 6 | 0/60 | 65.4927 | [65.4102, 65.5928] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-1w-dispatch-overhead` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0000 | [0.9979, 1.0005] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-2w-dispatch-overhead` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9989 | [0.9966, 1.0016] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-16w-dispatch-overhead` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9997 | [0.9939, 1.0057] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

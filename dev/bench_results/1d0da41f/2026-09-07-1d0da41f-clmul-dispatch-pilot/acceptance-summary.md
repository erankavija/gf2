# Acceptance summary for `pilot-1d0da41f-20260907t191830z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `da17c31c357951f4082e2b9c57eb581672309e0ef01ef2ac8a6ab0e251f0fbc0`.

Family `ymm-clmul-dispatch-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-small-8-1core` | Exploratory | SingleCore | 6 | 6 | 0/60 | 0.8054 | [0.7947, 0.8235] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-odd-tail-65-1core` | Exploratory | SingleCore | 6 | 6 | 0/60 | 0.6082 | [0.6067, 0.6152] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-l1-512-1core` | Exploratory | SingleCore | 6 | 6 | 0/60 | 0.5812 | [0.5801, 0.5815] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `fieldvec-dot-1024-1core` | Exploratory | SingleCore | 6 | 6 | 0/60 | 0.7725 | [0.7699, 0.7748] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

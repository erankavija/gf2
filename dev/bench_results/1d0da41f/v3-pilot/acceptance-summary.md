# Acceptance summary for `1d0da41f-v3-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `c69a28da7906dbb951ce93fa65a5b3c3e93d4fe58b27b92e12610fe2713b6b98`.

Family `ymm-clmul-dispatch`: 5 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-small-8-1core` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.7960 | [0.7622, 0.8118] at 0.9950 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-odd-tail-65-1core` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.6057 | [0.6050, 0.6804] at 0.9950 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-l1-512-1core` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5828 | [0.5643, 0.5831] at 0.9950 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `fieldvec-dot-1024-1core` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.7741 | [0.7687, 0.7764] at 0.9950 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

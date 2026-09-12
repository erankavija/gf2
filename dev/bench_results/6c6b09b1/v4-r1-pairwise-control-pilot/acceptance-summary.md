# Acceptance summary for `6c6b09b1-v4-r1-pairwise-control-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `b09e362338cd043a8eaa8dbbe234dfd1d43ca71af814bd38543af061ec0f5ebe`.

Family `byte-field-pairwise-control`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `pairwise-4k-batch-vs-gfcomplete` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5378 | [0.5355, 0.5428] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-batch-vs-gfcomplete` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5319 | [0.5311, 0.5423] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-whole-batch-vs-gfcomplete` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6890 | [0.6875, 0.6979] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-4k-batch-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4799 | [0.4788, 0.4824] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-batch-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4743 | [0.4739, 0.4843] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-whole-batch-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6153 | [0.6141, 0.6312] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-4k-batch-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1604 | [1.1186, 1.1801] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-batch-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8554 | [0.8224, 0.8643] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-whole-batch-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0795 | [1.0494, 1.1121] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

# Acceptance summary for `protocol-v3-r2-confirmation`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `d525a71b40721ccc4c08d7289eb11685215349bbd9d003df5d099c28fe4c8312`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `protocol-smoke-v3-r2`: 2 comparisons at family-wise alpha 0.025, per-comparison confidence 0.987500, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `decoder-pipeline` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0002 | [0.9992, 1.0012] at 0.9875 | NotWorse | **Pass** |  |
| `cold-first-use` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5004 | [0.4989, 0.5022] at 0.9875 | Regressed | **Fail** |  |

## Findings

No findings.

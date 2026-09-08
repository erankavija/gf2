# Acceptance summary for `protocol-v3-confirmation-r1`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `7c0bae02de0f76d2fffbfa25a16160c28e4deaaa398530a1289783b7d1d0429d`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `protocol-smoke-v3`: 2 comparisons at family-wise alpha 0.025, per-comparison confidence 0.987500, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `decoder-pipeline` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0004 | [0.9993, 1.0013] at 0.9875 | NotWorse | **Pass** |  |
| `cold-first-use` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5003 | [0.4995, 0.5023] at 0.9875 | Regressed | **Fail** |  |

## Findings

No findings.

# Acceptance summary for `6c6b09b1-v4-r1-region-axpy-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `e34bbc85e98faf97629229afd99f6204cc87e86c6e20d31b7c869b82abc7806e`.

Family `byte-field-region-axpy`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-4k-element-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 561.3050 | [558.6775, 564.1474] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-element-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 469.5247 | [466.4143, 471.4557] at 0.9958 | Improved | **Pass** |  |
| `axpy-8m-element-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 542.2688 | [539.8813, 543.4845] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-wide-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2231.0125 | [2198.6088, 2240.7322] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-whole-element-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 796.7365 | [790.4581, 807.1958] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-whole-wide-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2259.8443 | [2235.1141, 2269.5832] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.

# Acceptance summary for `67517a7b-ldpc-batch-decode-selector-holdout-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 6; resumed true.
Receipt digest `ec2684a288e9461c7be0eec4d68254a9aed6abe1464350d6b75cf891e159e7e6`.

Family `ldpc-batch-decode-selector-holdout-v1`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-tuned-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 5.2702 | [5.2314, 5.2847] at 0.9958 | Improved | **Pass** |  |
| `dvb-t2-r12-native-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9977 | [0.9674, 1.0189] at 0.9958 | NotWorse | **Pass** |  |
| `nr-bg1-z384-tuned-w1-b16` | Holdout | SingleCore | 0 | 24 | 0/240 | 5.5355 | [5.4343, 5.6132] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-tuned-w1-b5` | Holdout | SingleCore | 0 | 24 | 0/240 | 3.7866 | [3.6982, 3.8146] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-tuned-w1-b13` | Holdout | SingleCore | 0 | 24 | 0/240 | 4.5455 | [4.4328, 4.6685] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-native-w1-b16` | Holdout | SingleCore | 0 | 24 | 0/240 | 0.9920 | [0.9787, 1.0225] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.

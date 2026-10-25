# Acceptance summary for `d06a99ed-worker-routing-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `8d9d247f547a831929c42b3602c66275c3b0abff0a089b124604b97335e5a6b3`.

Family `d06a99ed-worker-aware-production-v1`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-production-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9841 | [0.9735, 0.9957] at 0.9958 | NotWorse | **Pass** |  |
| `dvb-t2-r12-production-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 8/240 | 0.9960 | [0.8232, 1.0432] at 0.9958 | Inconclusive | **Inconclusive** |  |
| `nr-bg1-z384-production-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9888 | [0.9850, 0.9921] at 0.9958 | NotWorse | **Pass** |  |
| `nr-bg1-z384-production-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9931 | [0.9849, 1.0027] at 0.9958 | NotWorse | **Pass** |  |
| `dvb-t2-r12-production-w12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 1.7399 | [1.4099, 1.7682] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-production-w24-b8` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 2.7900 | [2.7588, 2.8041] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.

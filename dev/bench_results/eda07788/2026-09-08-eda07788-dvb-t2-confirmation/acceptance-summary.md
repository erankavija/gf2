# Acceptance summary for `confirmation-eda07788-20260908t094903z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `7aa2efe0979c6482ec340a9966df3e80eb1406e773188c279ae793a7e6364073`.

Family `dvb-t2-bit-interleave-baselines`: 6 comparisons at family-wise alpha 0.05, per-comparison confidence 0.991667, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-gap-native-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4583 | [0.4536, 0.4666] at 0.9917 | Regressed | **Fail** |  |
| `dvb-t2-qam16-r12-normal-gap-portable-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4563 | [0.4533, 0.4579] at 0.9917 | Regressed | **Fail** |  |
| `dvb-t2-qam16-r12-normal-control-portable-vs-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9949 | [0.9744, 0.9994] at 0.9917 | NotWorse | **NotMaterial** |  |
| `dvb-t2-qam64-r12-normal-gap-native-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4302 | [0.4289, 0.4316] at 0.9917 | Regressed | **Fail** |  |
| `dvb-t2-qam64-r12-normal-gap-portable-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4266 | [0.4249, 0.4278] at 0.9917 | Regressed | **Fail** |  |
| `dvb-t2-qam64-r12-normal-control-portable-vs-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9932 | [0.9896, 0.9984] at 0.9917 | NotWorse | **NotMaterial** |  |

## Findings

No findings.

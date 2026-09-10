# Acceptance summary for `confirmation-v3-eda07788-20260908t194030z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `ab94fa3fcb4b9f54d4d236b1e80494767f8cc3e1141b063acc1d4342eeded52d`.

Family `dvb-t2-bit-interleave-baselines`: 12 comparisons at family-wise alpha 0.008333333333333333, per-comparison confidence 0.999306, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-gap-native-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4901 | [0.4879, 0.4937] at 0.9993 | Regressed | **NotConfirmatory** |  |
| `dvb-t2-qam16-r12-normal-gap-portable-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 1/240 | 0.4890 | [0.4297, 0.4954] at 0.9993 | Regressed | **NotConfirmatory** |  |
| `dvb-t2-qam16-r12-normal-control-portable-vs-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0018 | [0.9898, 1.0098] at 0.9993 | NotWorse | **NotConfirmatory** |  |
| `dvb-t2-qam64-r12-normal-gap-native-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4580 | [0.4510, 0.4626] at 0.9993 | Regressed | **NotConfirmatory** |  |
| `dvb-t2-qam64-r12-normal-gap-portable-vs-xdsopl` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4623 | [0.4443, 0.4702] at 0.9993 | Regressed | **NotConfirmatory** |  |
| `dvb-t2-qam64-r12-normal-control-portable-vs-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9993 | [0.9919, 1.0048] at 0.9993 | NotWorse | **NotConfirmatory** |  |

## Findings

- P-20 Note `dvb-t2-qam16-r12-normal-gap-native-vs-xdsopl`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `dvb-t2-qam16-r12-normal-gap-portable-vs-xdsopl`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `dvb-t2-qam16-r12-normal-control-portable-vs-native`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `dvb-t2-qam64-r12-normal-gap-native-vs-xdsopl`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `dvb-t2-qam64-r12-normal-gap-portable-vs-xdsopl`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `dvb-t2-qam64-r12-normal-control-portable-vs-native`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates

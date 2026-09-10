# Acceptance summary for `confirmation-c7113c5a-20260907t194909z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `63eb171e787ec773e934ef1bcf205f4f2f68e88dcba77c84fcabf3530018277e`.

Family `polynomial-multiplication-baselines`: 10 comparisons at family-wise alpha 0.05, per-comparison confidence 0.995000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `poly-mul-4w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6085 | [0.6040, 0.6151] at 0.9950 | Regressed | **Fail** |  |
| `poly-mul-9w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6625 | [0.6507, 0.6677] at 0.9950 | Regressed | **Fail** |  |
| `poly-mul-64w-1core` | Confirmatory | SingleCore | 0 | 24 | 119/240 | 167.4215 | [165.6136, 169.1429] at 0.9950 | Improved | **Unstable** |  |
| `poly-mul-256w-1core` | Confirmatory | SingleCore | 0 | 24 | 118/240 | 299.3836 | [293.9867, 303.0174] at 0.9950 | Improved | **Unstable** |  |
| `poly-mul-2048w-streaming-1core` | Confirmatory | SingleCore | 0 | 24 | 116/240 | 855.2000 | [847.5864, 865.3473] at 0.9950 | Improved | **Unstable** |  |
| `poly-mul-256w-6core` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 119/240 | 180.3740 | [168.5316, 188.7637] at 0.9950 | Improved | **Unstable** |  |
| `poly-mul-256w-12core` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 119/240 | 172.8281 | [161.5471, 182.8420] at 0.9950 | Improved | **Unstable** |  |
| `poly-mul-256w-24smt` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 117/240 | 166.9905 | [163.5956, 171.1376] at 0.9950 | Improved | **Unstable** |  |
| `clmul-batch-1024-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1491 | [0.1483, 0.1503] at 0.9950 | Regressed | **Fail** |  |
| `gf2m-dot-1024-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1536 | [0.1514, 0.1559] at 0.9950 | Regressed | **Fail** |  |

## Findings

No findings.

# Acceptance summary for `c7113c5a-v3-r1-baselines-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `14a6a55d8c71f0555c1dc37b2958a588d867e327ac73b38eb9244cbd3a40f225`.

Family `polynomial-multiplication-baselines`: 21 comparisons at family-wise alpha 0.008333333333333333, per-comparison confidence 0.999603, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `poly-mul-4w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6104 | [0.6081, 0.6159] at 0.9996 | Regressed | **NotConfirmatory** |  |
| `poly-mul-9w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6546 | [0.6518, 0.6578] at 0.9996 | Regressed | **NotConfirmatory** |  |
| `poly-mul-64w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 166.4978 | [164.8735, 167.4140] at 0.9996 | Improved | **NotConfirmatory** |  |
| `poly-mul-256w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 299.9931 | [299.0451, 301.3315] at 0.9996 | Improved | **NotConfirmatory** |  |
| `poly-mul-2048w-streaming-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 866.3559 | [860.5067, 871.8339] at 0.9996 | Improved | **NotConfirmatory** |  |
| `poly-mul-256w-6core` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 173.7302 | [158.3647, 181.2918] at 0.9996 | Improved | **NotConfirmatory** |  |
| `poly-mul-256w-12core` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 166.4594 | [138.7917, 190.0447] at 0.9996 | Improved | **NotConfirmatory** |  |
| `poly-mul-256w-24smt` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 167.3109 | [159.5583, 171.4505] at 0.9996 | Improved | **NotConfirmatory** |  |
| `poly-mul-4w-public-api-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 45.8539 | [45.6670, 46.1265] at 0.9996 | Improved | **NotConfirmatory** |  |
| `internal-clmul-batch-1024-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1538 | [0.1532, 0.1567] at 0.9996 | Regressed | **NotConfirmatory** |  |
| `internal-gf2m-dot-1024-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1568 | [0.1532, 0.1589] at 0.9996 | Regressed | **NotConfirmatory** |  |

## Findings

- P-20 Note `poly-mul-4w-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-9w-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-64w-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-256w-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-2048w-streaming-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-256w-6core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-256w-12core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-256w-24smt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `poly-mul-4w-public-api-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `internal-clmul-batch-1024-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `internal-gf2m-dot-1024-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates

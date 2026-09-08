# Acceptance summary for `confirmation-1d0da41f-20260908t084330z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `210ff5d6c7c07ac3a5bed2b713868f8f6a47375bbc6b2fd0a4d2f1ca660901c7`.

Family `ymm-clmul-dispatch`: 5 comparisons at family-wise alpha 0.05, per-comparison confidence 0.990000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-small-8-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7934 | [0.7792, 0.8117] at 0.9900 | Regressed | **Fail** |  |
| `raw-batch-odd-tail-65-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6052 | [0.6041, 0.6064] at 0.9900 | Regressed | **Fail** |  |
| `raw-batch-l1-512-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5854 | [0.5833, 0.5872] at 0.9900 | Regressed | **Fail** |  |
| `fieldvec-dot-1024-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7732 | [0.7683, 0.7780] at 0.9900 | Regressed | **Fail** |  |
| `raw-batch-l1-512-streaming-6core` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 0.5856 | [0.5827, 0.5872] at 0.9900 | Regressed | **Fail** |  |

## Findings

No findings.

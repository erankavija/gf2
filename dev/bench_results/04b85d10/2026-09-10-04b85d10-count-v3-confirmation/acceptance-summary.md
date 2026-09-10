# Acceptance summary for `confirmation-v3-count-04b85d10-20260910t162934z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `1b05b2d3ef37799902a045197a6154a8da447e49ccf6892ad205e3505ff44a19`.

Family `bit-storage-count-consumers`: 5 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `count-popcount-dispatch-4w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.5439 | [1.5392, 1.5481] at 0.9950 | Improved | **Pass** |  |
| `count-popcount-threshold-8w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2004 | [1.1953, 1.2063] at 0.9950 | Improved | **Pass** |  |
| `count-popcount-bandwidth-65536w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.0439 | [2.9160, 3.2251] at 0.9950 | Improved | **Pass** |  |
| `count-zero-test-507w-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1174 | [1.1111, 1.1207] at 0.9950 | Improved | **Pass** |  |
| `count-ldpc-check-64800-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9982 | [0.9956, 1.0029] at 0.9950 | NotWorse | **NotMaterial** |  |

## Findings

No findings.

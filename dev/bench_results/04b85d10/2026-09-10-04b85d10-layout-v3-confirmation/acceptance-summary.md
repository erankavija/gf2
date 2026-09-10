# Acceptance summary for `confirmation-v3-layout-04b85d10-20260910t163213z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `5e64f498e79ccd94eaa5f50096466a98e714ee551c2ab85244dddcfb4c1a9af3`.

Family `bit-storage-layout-consumers`: 5 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `layout-transpose-block-256-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.9276 | [1.9136, 1.9430] at 0.9950 | Improved | **Pass** |  |
| `layout-transpose-block-4096-6core` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 1.8932 | [1.8762, 1.9343] at 0.9950 | Improved | **Pass** |  |
| `layout-bch-encode-bitslice-m14-b256-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.4886 | [2.4830, 2.4933] at 0.9950 | Improved | **Pass** |  |
| `layout-bch-encode-fold-m14-b256-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.6985 | [2.6880, 2.7060] at 0.9950 | Improved | **Pass** |  |
| `layout-bch-encode-caller-buffer-m14-b256-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0023 | [0.9985, 1.0080] at 0.9950 | NotWorse | **NotMaterial** |  |

## Findings

No findings.

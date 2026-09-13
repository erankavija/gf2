# Acceptance summary for `pilot-v4-layout-04b85d10-20260913t010144z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `5a6f47f09b46f673802471a48a9767b99e5dd6dae73f5cb8c4bd753a091e416e`.

Family `bit-storage-layout-consumers`: 5 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `layout-bch-encode-bitslice-m14-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.4821 | [2.4789, 2.4868] at 0.9950 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-fold-m14-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.6972 | [2.6929, 2.7025] at 0.9950 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-caller-buffer-m14-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0027 | [1.0004, 1.0035] at 0.9950 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-bitslice-m16-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.4605 | [2.4571, 2.4642] at 0.9950 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-fold-m16-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.7143 | [2.7130, 2.7158] at 0.9950 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `layout-bch-encode-caller-buffer-m16-b256-1core` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0038 | [1.0032, 1.0045] at 0.9950 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

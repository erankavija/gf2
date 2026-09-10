# Acceptance summary for `popcount-26465e6c-20260908`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `ff2eba5f722beefea55e9216f1dc4f6f13f779644c38c54ac6370c544a3cd1e8`.

Family `popcount-baselines`: 9 comparisons at family-wise alpha 0.05, per-comparison confidence 0.994444, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-small-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.5738 | [1.5700, 1.5826] at 0.9944 | Improved | **Pass** |  |
| `popcount-boundary-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0882 | [1.0839, 1.0932] at 0.9944 | NotWorse | **NotMaterial** |  |
| `popcount-alignment-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3479 | [0.3464, 0.3516] at 0.9944 | Regressed | **Fail** |  |
| `popcount-bitpattern-allones-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.4775 | [1.4668, 1.4840] at 0.9944 | Improved | **Pass** |  |
| `popcount-bitpattern-allzero-vs-mula` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2019 | [1.1964, 1.2144] at 0.9944 | Improved | **Pass** |  |
| `popcount-csa-boundary-below-vs-mula` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7766 | [0.7721, 0.7822] at 0.9944 | Regressed | **Fail** |  |
| `popcount-csa-boundary-at-vs-mula` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2015 | [1.1947, 1.2074] at 0.9944 | Improved | **Pass** |  |
| `popcount-cache-resident-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3274 | [1.3244, 1.3338] at 0.9944 | Improved | **Pass** |  |
| `popcount-streaming-vs-mula` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1047 | [1.0860, 1.1252] at 0.9944 | NotWorse | **NotMaterial** |  |

## Findings

No findings.

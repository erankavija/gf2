# Acceptance summary for `26465e6c-v3-popcount-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 8; resumed true.
Receipt digest `81fef577d1ea03e58a0f4643fc8d15ccdb703778543433060e489d6eb6b0e52e`.

Family `popcount-baselines`: 9 comparisons at family-wise alpha 0.025, per-comparison confidence 0.997222, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-w4-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.3022 | [2.2526, 2.3770] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w4-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.5750 | [2.5149, 2.6220] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w4-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.8052 | [1.5898, 1.8533] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w4-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.5405 | [1.4957, 1.5631] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w4-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8914 | [0.8850, 0.9141] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.3620 | [1.3440, 1.3947] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2781 | [1.2674, 1.3072] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8672 | [0.8631, 0.8888] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8648 | [0.8535, 0.8808] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5668 | [0.5649, 0.5722] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w12-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2691 | [1.2628, 1.3210] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w12-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0668 | [1.0561, 1.0767] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w12-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6781 | [0.6744, 0.6830] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w12-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9281 | [0.9150, 0.9619] at 0.9972 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w12-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5684 | [0.5657, 0.5757] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w60-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0798 | [1.0744, 1.0930] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w60-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8931 | [0.8561, 0.8981] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w60-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4449 | [0.4435, 0.4479] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w60-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1673 | [1.1625, 1.1766] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w60-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6411 | [0.6382, 0.6518] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0822 | [1.0789, 1.0878] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8820 | [0.8275, 0.8956] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4332 | [0.4294, 0.4346] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1446 | [1.1341, 1.1528] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8838 | [0.8782, 0.8918] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0431 | [1.0412, 1.0476] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8230 | [0.8196, 0.8294] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3692 | [0.3670, 0.3699] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1749 | [1.1213, 1.1955] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9976 | [0.9937, 1.0018] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0218 | [1.0165, 1.0244] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.7949 | [0.7923, 0.7996] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3281 | [0.3263, 0.3309] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2793 | [1.2705, 1.2841] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1102 | [1.1050, 1.1180] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-off24-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0169 | [1.0101, 1.0198] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-off24-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.7984 | [0.7939, 0.8025] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-off24-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3307 | [0.3275, 0.3323] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w256-off24-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2517 | [1.2404, 1.2659] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-ones-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0811 | [1.0662, 1.0849] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-ones-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8860 | [0.8779, 0.8942] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-ones-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4335 | [0.4295, 0.4358] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-ones-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1458 | [1.1380, 1.1570] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-ones-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8906 | [0.8772, 0.8945] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-zeros-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0797 | [1.0724, 1.0851] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-zeros-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8855 | [0.8560, 0.8932] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-zeros-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4335 | [0.4309, 0.4350] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-zeros-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.1490 | [1.1363, 1.1586] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w64-zeros-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8927 | [0.8689, 0.8983] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9907 | [0.9883, 0.9994] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.7495 | [0.7478, 0.7597] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3101 | [0.3093, 0.3151] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.3573 | [1.3499, 1.3753] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2753 | [1.2708, 1.2930] at 0.9972 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1m-streaming-vs-nibble-lut` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0009 | [0.9950, 1.0072] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1m-streaming-vs-scalar-popcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9404 | [0.9167, 0.9620] at 0.9972 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1m-streaming-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5974 | [0.5912, 0.5999] at 0.9972 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1m-streaming-vs-libpopcnt` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0369 | [1.0174, 1.0556] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1m-streaming-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0501 | [1.0273, 1.0701] at 0.9972 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

# Acceptance summary for `26465e6c-v3-popcount-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 8; resumed true.
Receipt digest `2113fee2561eeab5303a99b1eadd015b68bff92fdbc9879bd9da330782f3d912`.

Family `popcount-baselines`: 68 comparisons at family-wise alpha 0.008333333333333333, per-comparison confidence 0.999877, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-w4-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.2964 | [2.1911, 2.3753] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w4-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.5626 | [2.3853, 2.6393] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w4-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.8004 | [1.5890, 1.8758] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w4-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.4967 | [1.4853, 1.5155] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w4-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8940 | [0.8871, 0.9162] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w8-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3630 | [1.3391, 1.4064] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w8-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2841 | [1.2626, 1.3229] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w8-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8732 | [0.8620, 0.8933] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w8-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8592 | [0.8517, 0.8890] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w8-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5657 | [0.5597, 0.5840] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w12-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2661 | [1.2529, 1.2857] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w12-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0594 | [1.0492, 1.0865] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w12-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6768 | [0.6729, 0.6847] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w12-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9254 | [0.9182, 0.9369] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w12-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5710 | [0.5674, 0.5834] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w60-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0773 | [1.0723, 1.0808] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w60-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8881 | [0.8777, 0.8937] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w60-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4443 | [0.4422, 0.4457] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w60-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1636 | [1.1586, 1.1676] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w60-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6396 | [0.6360, 0.6431] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w64-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0821 | [1.0766, 1.0868] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w64-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8824 | [0.8467, 0.8945] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w64-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4338 | [0.4300, 0.4358] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w64-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1486 | [1.1416, 1.1549] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w64-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8909 | [0.8846, 0.8961] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w128-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0400 | [1.0354, 1.0488] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w128-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8229 | [0.8162, 0.8303] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w128-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3704 | [0.3645, 0.3731] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w128-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1858 | [1.1736, 1.1962] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w128-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9974 | [0.9879, 1.0051] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w256-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0183 | [1.0061, 1.0313] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w256-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7968 | [0.7912, 0.8033] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w256-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3286 | [0.3270, 0.3307] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w256-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2793 | [1.2725, 1.2832] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w256-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1084 | [1.0956, 1.1117] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w256-off24-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0180 | [1.0154, 1.0206] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w256-off24-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7972 | [0.7942, 0.7999] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w256-off24-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3307 | [0.3285, 0.3319] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w256-off24-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2561 | [1.2465, 1.2644] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w64-ones-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0806 | [1.0709, 1.0872] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w64-ones-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8829 | [0.8737, 0.8928] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w64-ones-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4328 | [0.4300, 0.4343] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w64-ones-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1473 | [1.1356, 1.1576] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w64-ones-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8928 | [0.8835, 0.8988] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w64-zeros-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0763 | [1.0711, 1.0828] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w64-zeros-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8820 | [0.8722, 0.8909] at 0.9999 | Inconclusive | **NotConfirmatory** |  |
| `popcount-w64-zeros-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4327 | [0.4294, 0.4365] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w64-zeros-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1459 | [1.1358, 1.1535] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w64-zeros-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8915 | [0.8855, 0.8980] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w16384-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9883 | [0.9839, 1.0041] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w16384-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7502 | [0.7468, 0.7543] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w16384-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3103 | [0.3087, 0.3137] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w16384-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3618 | [1.3534, 1.3723] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w16384-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2860 | [1.2758, 1.2950] at 0.9999 | Improved | **NotConfirmatory** |  |
| `popcount-w1m-streaming-vs-nibble-lut` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0000 | [0.9798, 1.0095] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w1m-streaming-vs-scalar-popcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9547 | [0.9226, 0.9637] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w1m-streaming-vs-compiler-count-ones` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6065 | [0.5899, 0.6129] at 0.9999 | Regressed | **NotConfirmatory** |  |
| `popcount-w1m-streaming-vs-libpopcnt` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0287 | [1.0135, 1.0623] at 0.9999 | NotWorse | **NotConfirmatory** |  |
| `popcount-w1m-streaming-vs-mula-avx2-harley-seal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0587 | [1.0334, 1.0701] at 0.9999 | NotWorse | **NotConfirmatory** |  |

## Findings

- P-20 Note `popcount-w4-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w4-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w4-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w4-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w4-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w8-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w8-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w8-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w8-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w8-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w12-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w12-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w12-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w12-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w12-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w60-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w60-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w60-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w60-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w60-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w128-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w128-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w128-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w128-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w128-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-off24-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-off24-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-off24-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w256-off24-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-ones-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-ones-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-ones-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-ones-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-ones-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-zeros-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-zeros-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-zeros-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-zeros-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w64-zeros-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w16384-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w16384-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w16384-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w16384-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w16384-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w1m-streaming-vs-nibble-lut`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w1m-streaming-vs-scalar-popcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w1m-streaming-vs-compiler-count-ones`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w1m-streaming-vs-libpopcnt`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `popcount-w1m-streaming-vs-mula-avx2-harley-seal`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates

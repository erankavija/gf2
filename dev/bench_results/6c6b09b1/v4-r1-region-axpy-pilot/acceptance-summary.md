# Acceptance summary for `6c6b09b1-v4-r1-region-axpy-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `36c76bea9e8c61bb80e6ba1bd230fec9c05389962653e3e24a428ce3a0384393`.

Family `byte-field-region-axpy`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-4k-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 564.7750 | [505.2643, 566.0665] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 473.8743 | [470.8632, 474.4831] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-8m-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 545.4776 | [544.5605, 546.4242] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-2m-stream-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 305.2347 | [301.1363, 307.1286] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-whole-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 891.1697 | [885.5842, 893.7360] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-whole-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 808.0524 | [804.0558, 811.3003] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2648.2137 | [2642.4137, 2669.2479] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2213.5210 | [2190.4629, 2241.4833] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-8m-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2244.6520 | [2114.7672, 2250.3268] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-2m-stream-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1447.9386 | [1434.7418, 1463.8776] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-whole-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2461.6858 | [2433.3745, 2470.5545] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-whole-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2260.9391 | [2224.0994, 2268.8121] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-isal-vs-gfcomplete` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5005 | [0.4976, 0.5223] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-isal-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0181 | [0.0180, 0.0182] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-isal-vs-gfcomplete` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6762 | [0.6692, 0.6834] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-isal-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0222 | [0.0218, 0.0223] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-element-vs-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.2104 | [0.2099, 0.2110] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

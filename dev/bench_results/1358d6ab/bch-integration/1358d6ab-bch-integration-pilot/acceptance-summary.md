# Acceptance summary for `1358d6ab-bch-integration-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 20; resumed true.
Receipt digest `6877f32770be24621a5924dd37730ab772781690b33821483047130aa6b4ef63`.

Family `1362381c-bch-integration`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `m14-b1-aff3ct-lfsr-scalar` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2674 | [0.2669, 0.2684] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m14-b16-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4426 | [0.4421, 0.4432] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m14-b256-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4386 | [0.3834, 0.4401] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m14-b4096-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.5641 | [0.5528, 0.5647] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b1-aff3ct-lfsr-scalar` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0700 | [1.0639, 1.0719] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b16-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.1413 | [2.1053, 2.1455] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b256-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.6495 | [1.6419, 1.6633] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b4096-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.6342 | [1.6285, 1.6352] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b1-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 106.0980 | [105.6093, 106.7946] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b16-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 107.2942 | [106.9614, 108.0483] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b256-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 107.5102 | [107.1842, 108.5382] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2s-b4096-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 107.3544 | [107.1723, 107.5520] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b1-aff3ct-lfsr-scalar` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.8954 | [0.8929, 0.9031] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b16-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.4176 | [1.3743, 1.4383] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b256-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.7650 | [1.7544, 1.7773] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b4096-aff3ct-lfsr-simd-inter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.7704 | [1.7537, 1.7892] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b1-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 104.5141 | [104.1353, 104.7695] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b16-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 104.7675 | [104.4428, 105.5346] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b256-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 106.1224 | [105.9366, 107.2535] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `t2n-b4096-bchlib-table-remainder` | Exploratory | SingleCore | 0 | 24 | 0/240 | 106.2278 | [106.1474, 107.4741] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.

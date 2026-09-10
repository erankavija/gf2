# Acceptance summary for `6fb89a3c-v3-r1-logical-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 5; resumed true.
Receipt digest `2857b9ddadcc8aa579aeaa047c16ee7a40c0d76e514e9779c4df7090de1b8069`.

Family `logical-buffer-vs-isal`: 9 comparisons at family-wise alpha 0.025, per-comparison confidence 0.997222, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `logical-xor-7w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1951 | [0.1938, 0.1965] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-8w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1368 | [0.1354, 0.1375] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-9w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1581 | [0.1568, 0.1589] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-63w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0466 | [0.0463, 0.0476] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-64w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0407 | [0.0406, 0.0413] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-65w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0433 | [0.0429, 0.0439] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-16w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0759 | [0.0755, 0.0764] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-xor-32w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0513 | [0.0510, 0.0539] at 0.9972 | Regressed | **NotConfirmatory** |  |
| `logical-parity-3src-64w-vs-isal-base` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0424 | [0.0423, 0.0428] at 0.9972 | Regressed | **NotConfirmatory** |  |

## Findings

- P-20 Note `logical-xor-7w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-8w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-9w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-63w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-64w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-65w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-16w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-xor-32w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `logical-parity-3src-64w-vs-isal-base`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates

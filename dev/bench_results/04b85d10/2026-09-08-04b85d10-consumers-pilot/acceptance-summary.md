# Acceptance summary for `pilot-04b85d10-20260908t085658z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `df45ad5784a356701fda313221d072b1c2cc48433c090fc63c684cbc644b2060`.

Family `bit-storage-consumers-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `logical-row-xor-dispatch-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.0861 | [1.0652, 1.1088] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `logical-row-xor-threshold-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.3360 | [1.3294, 1.3463] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `logical-dense-rref-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.0051 | [1.0032, 1.0092] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |
| `logical-ldpc-syndrome-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 0.9953 | [0.9864, 1.0099] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |
| `count-popcount-dispatch-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.5042 | [1.5028, 1.5092] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `count-popcount-threshold-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.1661 | [1.1601, 1.1689] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `count-zero-test-syndrome-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.1241 | [1.1215, 1.1285] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `count-ldpc-check-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.0043 | [0.9935, 1.0097] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `layout-transpose-block-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.9281 | [1.9071, 1.9441] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `layout-bch-encode-bitslice-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 2.4841 | [2.4814, 2.4930] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `layout-bch-encode-fold-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 2.7344 | [2.7297, 2.7377] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `layout-bch-encode-alloc-1core` | Exploratory | SingleCore | 0 | 8 | 0/80 | 1.0013 | [0.9987, 1.0019] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |
| `layout-transpose-block-6core` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 8 | 0/80 | 1.9583 | [1.9261, 2.0087] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `layout-bch-encode-parallel-12core` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 8 | 0/80 | 0.9790 | [0.8945, 1.1028] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |
| `layout-bch-encode-parallel-24logical` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 8 | 0/80 | 1.0142 | [0.9221, 1.0639] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |

## Findings

No findings.

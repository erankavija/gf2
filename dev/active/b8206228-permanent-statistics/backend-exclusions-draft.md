# Draft backend exclusion records

Status: draft. These records explain why a candidate does not enter rule 3 for each field family. They do not turn a missing timing cohort into a selection.

## Rule 1: behavioral and equivalence exclusions

The committed shared-equivalence receipts report per-matrix agreement against the field reference for the candidates that execute at their listed orders. A candidate with no cell row in the committed equivalence receipt has no rule-1 passage for that cell. The GPU study receipt records q3, q5, and q7 equivalence rows at its measured orders; the b488f02c equivalence receipt is an older supporting check. These receipts do not replace the missing same-cohort timing receipt.

## Rule 2: safety and capability exclusions

- $\mathbb{F}_5$: AVX2 and intra-matrix Rayon are unsupported in the in-tree candidate registry; the study receipt records those candidates as unsupported by library capability. This is a capability exclusion, not a GPU resource falsification.
- $\mathbb{F}_7$, $n>16$: packed scalar, packed Rayon, and AVX2 paths are excluded by the documented 16-lane limit; the generic Ryser and accelerator paths remain the candidate family where their rule-1 evidence exists.
- GPU candidates: the study resource receipts record the AMD Radeon RX 6950 XT `gfx1030`, ROCm 7.2.4, driver/kernel identity, and launch/resource observations. A GPU study row remains non-comparable to the 296a cohort when its source, binary, workload shape, or timing protocol differs.
- Any candidate absent from a cell-applicable committed equivalence row is excluded by rule 1 for that cell until a matching validation/equivalence receipt is committed.

## Rule 3 exclusions

The b488f02c rows are not ranked against either the 296a41c9 cohort or the GPU-study cohorts. They differ in source/build identity and timing protocol, and the envelope includes projected rows without measured composite rates. The determinant-cost receipt measures evaluation-only marginal cost. These facts exclude those rows from rule-3 selection; they do not establish that a backend is mathematically or mechanically invalid.

The GPU-study grids are nomination evidence only. Their source revision, binary set, schedule, and timing protocol differ from 296a41c9, so their rows do not discharge or cross-rank any cell. The q3 no-go in `dev/studies/0dffa759/findings.md` §3 excludes retention of q3 n=16,20,24 device rows; §3.2 preserves the confounded `fold-gf3` finding. The owner decision dated 2026-08-17 excludes the retained `f5-three-plane` and `f7-three-plane-permanent` study prototypes from freeze selection while keeping their factual inventory rows. The 296a41c9 cohort discharges only q3 n=28, q5 n=24, and q7 n=20.

The q3 n=16,20,24 rows therefore use `generic_ryser` placeholders in the manifest, selection receipt, and inventory, and they are gap rows. They do not carry a retained accelerator configuration from the GPU study.

The only-one-eligible path applies to $(q,n)=(5,24)$ and $(7,20)$ in the current 296a cohort. The ranked path applies only to $(3,28)$. For every one of the 60 gap cells, exactly two candidates are nominated in `gap-list.md`; every eligible but non-nominated candidate is recorded as a rule-3 exclusion for that cell. The three forced nominations are q7 n=17,18,19, where capability leaves only `accelerator` and `generic_ryser`.

## Per-gap rule-3 exclusion ledger

| Gap cells | Eligible candidate count | Nominated count | Non-nominated rule-3 exclusions |
|---|---:|---:|---:|
| q3 n=4..15 | 7 | 2 | 60 |
| q3 n=16..27, including n=16,20,24 | 7 | 2 | 60 |
| q5 n=4..23 | 4 | 2 | 40 |
| q7 n=4..16 | 4 | 2 | 26 |
| q7 n=17..19 (capability-forced pair) | 2 | 2 | 0 |
| **Total** |  |  | **186** |

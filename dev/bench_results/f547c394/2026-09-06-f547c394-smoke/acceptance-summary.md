# Acceptance summary for `smoke-f547c394-20260906t213607z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `b066dd3f61e6cfb97744845df6e7dc950ef5e33ef9e65aba7c514d5c8c022055`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `protocol-smoke`: 3 comparisons at family-wise alpha 0.05, per-comparison confidence 0.983333, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-fold-double-pass-1core` | Confirmatory | SingleCore | 6 | 24 | 0/240 | 2.0000 | [1.9926, 2.0113] at 0.9833 | Improved | **Pass** |  |
| `xor-fold-identical-1core` | Confirmatory | SingleCore | 6 | 24 | 0/240 | 0.9975 | [0.9485, 1.0026] at 0.9833 | NotWorse | **Pass** |  |
| `xor-fold-streaming-pilot-6core` | Exploratory | PhysicalCores6 | 6,7,8,9,10,11 | 6 | 0/60 | 0.9794 | [0.9627, 0.9976] at 0.9833 | NotWorse | **Pilot** |  |
| `xor-fold-identical-12core` | Confirmatory | PhysicalCores12 | none | 0 | 0/0 | n/a | n/a | n/a | **Unavailable** | affinity mask holds 6 physical cores, 12 required |

## Findings

- P-02 Note: protocol changed in the working tree after this measurement; the pinned version governed the run

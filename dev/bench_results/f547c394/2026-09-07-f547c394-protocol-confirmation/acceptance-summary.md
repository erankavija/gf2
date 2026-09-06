# Acceptance summary for `confirmation-f547c394-20260906t230920z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `e87a36d0358c3583e35125e53b9ae481a9ec6afa21949a64108b081314d35344`.

Family `protocol-smoke`: 3 comparisons at family-wise alpha 0.05, per-comparison confidence 0.983333, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-fold-double-pass-1core` | Confirmatory | SingleCore | 6 | 24 | 0/240 | 1.9993 | [1.9066, 2.0536] at 0.9833 | Improved | **Pass** |  |
| `xor-fold-identical-1core` | Confirmatory | SingleCore | 6 | 24 | 0/240 | 1.0371 | [0.9862, 1.0897] at 0.9833 | NotWorse | **Pass** |  |
| `xor-fold-streaming-pilot-6core` | Exploratory | PhysicalCores6 | 6,7,8,9,10,11 | 6 | 0/60 | 0.9861 | [0.9613, 1.0108] at 0.9833 | NotWorse | **Pilot** |  |
| `xor-fold-identical-12core` | Confirmatory | PhysicalCores12 | none | 0 | 0/0 | n/a | n/a | n/a | **Unavailable** | affinity mask holds 6 physical cores, 12 required |

## Findings

No findings.

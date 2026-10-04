# Acceptance summary for `v4-r1-confirmation-2037941f-dense-matvec-vs-m4ri`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `bc3c5fa640f1857576ea7f8bbc80b4e82fedc5374cf491931f2826e5439cc838`.

Family `2037941f-dense-matvec-vs-m4ri`: 2 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.0125 per comparison (confidence 0.987500), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `m4ri-gap-65x512-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0028 | [0.0028, 0.0028] at 0.9875 | Regressed | **Fail** |  |
| `m4ri-gap-65x4096-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0009] at 0.9875 | Regressed | **Fail** |  |

## Findings

No findings.

# ISA-L scalar-gap pilot stopping result

> **Diátaxis Type:** Reference

Source: `dev/bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/receipt.json` (SHA-256 `b479513125bf558cfe4bb70d31697e089a60d86ba1f8b17dd5bcdfbbfc05d009`); frozen rule: `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md`.

The canonical `survey-analysis resolution` command recomputes whole-pair bootstrap intervals at the first confirmation's corrected alpha.

Command: `survey-analysis resolution dev/bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot 0.005`

First attempt: $t=1$, $m=5$, $\alpha_c=0.005$; bootstrap tail support is 25 expected draws per tail.

| Primary warm cell | Relative half-width |
|---|---:|
| `isal-base-gap-8w-a64-warm` | 0.057694 |
| `isal-base-gap-9w-a64-warm` | 0.064228 |
| `isal-base-gap-63w-a64-warm` | 0.055812 |
| `isal-base-gap-64w-a64-warm` | 0.086869 |
| `isal-base-gap-65w-a64-warm` | 0.480138 |

Largest primary width: 0.480138; frozen upward rounding to 0.001: $r=0.481$. Family ceiling: 0.030.

**Outcome: `resolution-insufficient`.** The frozen family rule ends this comparison after its exploratory pilot. No confirmatory reservation is made.

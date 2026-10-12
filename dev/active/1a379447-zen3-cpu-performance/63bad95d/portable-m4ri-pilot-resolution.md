# Portable comparator pilot resolution

> **Diátaxis Type:** Reference

Source: `dev/bench_results/63bad95d/63bad95d-portable-matvec-vs-m4ri/v4-r1-pilot/receipt.json` (SHA-256 `c4e96aa34746dd16fe153532947b94e59ded6df464d824cda8a5661b0573952d`); frozen rule: `dev/active/1a379447-zen3-cpu-performance/63bad95d/portable-m4ri-addendum.md`.

The canonical `survey-analysis resolution` command recomputes whole-pair bootstrap intervals at the first confirmation's corrected alpha.

Command: `survey-analysis resolution dev/bench_results/63bad95d/63bad95d-portable-matvec-vs-m4ri/v4-r1-pilot 0.00416666666667`

Ledger `dev/bench_results/63bad95d/portable-matvec-vs-m4ri-ledger.jsonl` through the pilot: 1 reservation(s), 0 reserved comparison(s), so the confirmation is attempt $t=1$ with $\alpha_t=0.025$ (`dev/tools/tuning-campaign-support/src/trial_ledger.rs`). P-20 requires 20 expected bootstrap draws per tail (`dev/tools/tuning-campaign-support/src/receipt.rs`), which admits 6 further comparison(s) at 10000 resamples. The family's $m=6$ confirmatory cells give $\alpha_c=0.00416667$ and 20.8333 expected draws per tail.

| Confirmatory cell | Pilot pairs | Relative half-width |
|---|---:|---:|
| `portable-m4ri-gap-65x512-warm` | 24 | 0.022951 |
| `portable-m4ri-gap-65x4096-warm` | 24 | 0.402187 |
| `portable-m4ri-gap-65x512-retained-warm` | 24 | 0.039338 |
| `portable-m4ri-gap-65x4096-retained-warm` | 24 | 0.011586 |
| `portable-m4ri-gap-1x1-warm` | 24 | 0.008024 |
| `portable-m4ri-gap-64x64-warm` | 24 | 0.217945 |

Largest confirmatory width: 0.402187; frozen upward rounding to 0.001: $r=0.403$. Family ceiling: 0.040.

**Outcome: `resolution-insufficient`.** The frozen family rule ends this comparison after its exploratory pilot. No confirmatory reservation is made.

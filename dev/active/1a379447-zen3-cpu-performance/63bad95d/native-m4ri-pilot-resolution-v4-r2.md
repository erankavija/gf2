# Native comparator pilot resolution

> **Diátaxis Type:** Reference

Source: `dev/bench_results/63bad95d/63bad95d-native-matvec-vs-m4ri/v4-r2-pilot/receipt.json` (SHA-256 `351e2fa3583e58151f9d9803f06f4a69e3e4d0b24156cb3efab2e1a9e68cfce6`); frozen rule: `dev/active/1a379447-zen3-cpu-performance/63bad95d/native-m4ri-addendum.md`.

The canonical `survey-analysis resolution` command recomputes whole-pair bootstrap intervals at the first confirmation's corrected alpha.

Command: `survey-analysis resolution dev/bench_results/63bad95d/63bad95d-native-matvec-vs-m4ri/v4-r2-pilot 0.00416666666667`

Ledger `dev/bench_results/63bad95d/native-matvec-vs-m4ri-ledger.jsonl` through the pilot: 2 reservation(s), 0 reserved comparison(s), so the confirmation is attempt $t=1$ with $\alpha_t=0.025$ (`dev/tools/tuning-campaign-support/src/trial_ledger.rs`). P-20 requires 20 expected bootstrap draws per tail (`dev/tools/tuning-campaign-support/src/receipt.rs`), which admits 6 further comparison(s) at 10000 resamples. The family's $m=6$ confirmatory cells give $\alpha_c=0.00416667$ and 20.8333 expected draws per tail.

| Confirmatory cell | Pilot pairs | Relative half-width |
|---|---:|---:|
| `native-m4ri-gap-65x512-warm` | 24 | 0.002559 |
| `native-m4ri-gap-65x4096-warm` | 24 | 0.002125 |
| `native-m4ri-gap-65x512-retained-warm` | 24 | 0.003653 |
| `native-m4ri-gap-65x4096-retained-warm` | 24 | 0.001642 |
| `native-m4ri-gap-1x1-warm` | 24 | 0.012333 |
| `native-m4ri-gap-64x64-warm` | 24 | 0.006308 |

Largest confirmatory width: 0.012333; frozen upward rounding to 0.001: $r=0.013$. Family ceiling: 0.040.

**Outcome: `eligible`.** The pilot resolves the family within its frozen ceiling. The canonical freezer pins this resolution in the confirmation addendum.

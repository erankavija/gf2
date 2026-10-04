# M4RI comparator pilot resolution

> **Diátaxis Type:** Reference

Source: `dev/bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/receipt.json` (SHA-256 `714223d50a6774a7a793db6848c162e45b29958672fc12d807e94ad7d61ba9ab`); frozen rule: `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md`.

The canonical `survey-analysis resolution` command recomputes whole-pair bootstrap intervals at the first confirmation's corrected alpha.

Command: `survey-analysis resolution dev/bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot 0.0125`

Ledger `dev/bench_results/2037941f/dense-matvec-vs-m4ri-ledger.jsonl` through the pilot: 1 reservation(s), 0 reserved comparison(s), so the confirmation is attempt $t=1$ with $\alpha_t=0.025$ (`dev/tools/tuning-campaign-support/src/trial_ledger.rs`). P-20 requires 20 expected bootstrap draws per tail (`dev/tools/tuning-campaign-support/src/receipt.rs`), which admits 6 further comparison(s) at 10000 resamples. The family's $m=2$ confirmatory cells give $\alpha_c=0.0125$ and 62.5 expected draws per tail.

| Confirmatory cell | Pilot pairs | Relative half-width |
|---|---:|---:|
| `m4ri-gap-65x512-warm` | 24 | 0.002158 |
| `m4ri-gap-65x4096-warm` | 24 | 0.011641 |

Largest confirmatory width: 0.011641; frozen upward rounding to 0.001: $r=0.012$. Family ceiling: 0.040.

**Outcome: `eligible`.** The pilot resolves the family within its frozen ceiling. The canonical freezer pins this resolution in the confirmation addendum.

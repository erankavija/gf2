## Review: Make caller-trusted safe kernel entry points checked or unchecked by name (bfc2ceab)

**Verdict:** FAIL (rework allowance exhausted; escalated)

### Gate status

cargo-ci passed on the merged tree of rework 2; code-review failed.

### Prior-findings regression table (Tier 1.5)

| Round | Finding | Status at HEAD |
|-------|---------|----------------|
| R1 | F1 safe SpMM entries panic for `n == 0` with stored columns | closed (reviewer R2) |
| R1 | F2 per-product SpMM check without a committed cost record | closed (reviewer R2; invoker accepted the exploratory cost) |
| R2 | F1 PAM contract test covers the detected backend only | closed (reviewer R3): `crates/gf2-kernels-simd/tests/entry_conditions.rs` (`pam_cases`) |
| R2 | F2 arm manifests reach the support crate by directory climb | closed (reviewer R3): exception `@/issue/e430f035` cited in `bfc2ceab/verification.md` |

### Success criteria

No round-3 finding names a criterion; the three findings cite
`@/invariant/convention-convergence` and `@/invariant/non-obvious-comments`.

### Required changes

1. R3 F1: `bfc2ceab/survey/make-spmm-pilot-pairs.py` reimplements the RNG, the
   bootstrap seed, the estimator and the percentile bootstrap of
   `tuning-campaign-support::abtest`.
2. R3 F2: `dev/bench_results/bfc2ceab/run-spmm-check.sh` and
   `run-entry-check.sh` carry private `live()` locators beside
   `dev/scripts/repository_files.py`.
3. R3 F3: module introductions of `dev/scripts/campaign_plan.py`,
   `bfc2ceab/survey/make-inventory.py` and
   `bfc2ceab/survey/make-spmm-freeze-refusal.py` exceed one short paragraph.

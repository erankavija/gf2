# 112f23c6 verdict baseline

Receipt acceptance verdicts and the crate test result before and after the
shared `repository_root` resolver. The verdict lines of both runs are
byte-identical.

Baseline commit: `6776fe30ea9af85ad509ddca95926f051421676d`.

## Commands

Build the verifier, then evaluate scratch copies of every committed receipt
directory that carries an `acceptance-summary.json` (the verifier writes its
summary beside the receipt, so committed directories stay untouched):

```
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
dev/active/fa787f85-documentation-overhaul/112f23c6-verdicts.sh <outfile>
```

Test execution (the subset `scripts/cargo-ci.sh` runs as
`tuning-campaign-support-nextest`):

```
./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --cargo-profile ci-test --profile ci
```

## Results

| Measure | Baseline | After |
| --- | --- | --- |
| Receipt directories evaluated | 147 | 147 |
| `Accepted` | 137 | 137 |
| `Rejected` | 9 | 9 |
| Verifier usage error | 1 | 1 |
| `tuning-campaign-support` tests | 208 passed, 0 failed | 208 passed, 0 failed |

Every evaluated directory with its verdict line is in
`112f23c6-verdicts.txt`; the baseline and after outputs are byte-identical
(`diff` empty). The nine `Rejected` lines include two snapshot directories
nested under `inputs/producing/`; the usage-error line is
`c077a88b/v3-r1-matched-confirmation-rejection`, whose scratch copy the
verifier cannot open.

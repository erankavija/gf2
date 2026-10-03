# 112f23c6 verdict baseline

Receipt acceptance verdicts and crate test results of `tuning-campaign-support`
before and after the shared `repository_root` resolver. Raw outputs are
committed beside this record.

Baseline source: `6776fe30ea9af85ad509ddca95926f051421676d`. After source:
`21950e7bd189258e628fb33e560c6f470eb70abd`.

## Commands

Run at each source state. The verifier writes its summary beside the receipt,
so the runner evaluates scratch copies of every committed directory that
carries an `acceptance-summary.json`:

```
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
dev/active/fa787f85-documentation-overhaul/112f23c6-verdicts.sh <verdict-file>
./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --cargo-profile ci-test --profile ci > <nextest-file> 2>&1
```

The nextest invocation is the `tuning-campaign-support-nextest` step of
`scripts/cargo-ci.sh`.

## Raw outputs

| State | Verdicts | Nextest |
| --- | --- | --- |
| Baseline | `112f23c6-verdicts-before.txt` | `112f23c6-nextest-before.txt` |
| After | `112f23c6-verdicts-after.txt` | `112f23c6-nextest-after.txt` |

## Equality

The verdict listings are identical; this command prints nothing:

```
diff dev/active/fa787f85-documentation-overhaul/112f23c6-verdicts-before.txt dev/active/fa787f85-documentation-overhaul/112f23c6-verdicts-after.txt
```

Both nextest runs execute the same tests with the same outcomes; this command
prints nothing:

```
r() { grep -E '^ +(PASS|FAIL)' "$1" | sed -E 's/\[ *[0-9.]+s\] \([ 0-9]+\/[0-9]+\) //' | sort; }
diff <(r dev/active/fa787f85-documentation-overhaul/112f23c6-nextest-before.txt) <(r dev/active/fa787f85-documentation-overhaul/112f23c6-nextest-after.txt)
```

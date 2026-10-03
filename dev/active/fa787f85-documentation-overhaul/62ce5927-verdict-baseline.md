# 62ce5927 verdict baseline

Benchmark-acceptance verdicts and findings of every committed `receipt.json`
with schema `zen3-benchmark-receipt-v1` outside `inputs/` directories, before
and after campaign declaration and live shared-document lookups count
byte-identical copies as one file. Raw outputs are committed beside this
record.

Baseline source: `e3bfdfae782161c1f5031daac98f77ef48db2281`. After source:
`fb8620a819fb34d85d2e8dd73a3b99549296c351`.

## Commands

Run from the repository root at each source state. `R` is this record's
directory. The verifier writes its summary beside the receipt, so the runner
evaluates a scratch copy of each receipt directory and lists the verdict line
and every finding of its summary:

```
R=$(dirname "$(git ls-files ':(glob)**/62ce5927-verdict-baseline.md')")
CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
"$R/62ce5927-verdicts.sh" <verdict-file>
```

## Raw outputs

| State | Verdicts and findings |
| --- | --- |
| Baseline | `62ce5927-verdicts-before.txt` |
| After | `62ce5927-verdicts-after.txt` |

## Equality

The listings are identical; this command prints nothing:

```
diff "$R/62ce5927-verdicts-before.txt" "$R/62ce5927-verdicts-after.txt"
```

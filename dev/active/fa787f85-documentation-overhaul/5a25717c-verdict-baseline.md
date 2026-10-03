# 5a25717c verdict baseline

Benchmark-acceptance verdicts and findings of every committed `receipt.json`
with schema `zen3-benchmark-receipt-v1` outside `inputs/` directories, before
and after content-identity matching of the shared receipt pins. Raw outputs
are committed beside this record.

Baseline source: `65a916ade2a38529ddbe40b4bb4a72f1645e17f5`, listed by the
script as committed in `a06a73ad5baaf2259d4e3b13f3bfca44abe3a1bb`, which
selects receipts by a path scope. After source:
`92fa3e22895d9f6d02f7acabaf778d4318336f20`, listed by the script as committed
with this record, which selects receipts by schema among all tracked files.

## Commands

Run at each source state. The verifier writes its summary beside the receipt,
so the runner evaluates a scratch copy of each receipt directory and lists the
verdict line and every finding of its summary:

```
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts.sh <verdict-file>
```

## Raw outputs

| State | Verdicts and findings |
| --- | --- |
| Baseline | `5a25717c-verdicts-before.txt` |
| After | `5a25717c-verdicts-after.txt` |

## Equality

The listings are identical; this command prints nothing:

```
diff dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts-before.txt dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts-after.txt
```

## Protocol text

`protocol.md` keeps its bytes. These commands list the committed receipts that
pin its digest and the producing manifests that select it:

```
git grep -l "$(sha256sum dev/active/f547c394/protocol.md | cut -d' ' -f1)" -- ':(glob)**/receipt.json' ':(exclude,glob)**/inputs/**'
git grep -l 'f547c394/protocol.md' -- ':(glob)**/*producing*.json' ':(exclude,glob)**/inputs/**'
```

P-02 in `protocol.md` states path matching; the implementation matches content
identity.

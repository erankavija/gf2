# Corrections to the surveyed reports

> **Diátaxis Type:** Reference

Record of every conclusion that the citation pass changes in
`dev/active/c077a88b/findings.md`, `dev/active/1d0da41f/findings.md` and
`dev/active/eda07788/findings.md`. The inventories beside this file
(`inventory-<issue>.json`) carry the full statement-by-statement audit; this
file holds only the cases where a hand-copied number and its authoritative
artifact disagree, together with the effect on the conclusion the report draws.

## Corrections

None. Every quantitative statement in the three reports reproduces the value its
authoritative artifact holds, to the precision the statement prints. The three
`inventory-<issue>.json` files record the artifact and location checked for each
statement and carry an empty `disagreements` list.

Every conclusion the three reports stated before the citation pass is still
stated afterwards. The pass removes restatements, not findings.

## Scope clarifications

These statements are exact as written but read as though they covered more rows
than they do. The rewrite makes the scope explicit and points at the table, where
the neighbouring rows are visible. No conclusion changes.

| Report | Statement | Artifact reading | Clarification |
|---|---|---|---|
| `dev/active/eda07788/findings.md` | "The conversion spans exceed the paired call gap in 24 of 24 pairs on 16-QAM and in 0 of 24 on 64-QAM", inside the withdrawn confirmation's controls-and-attribution paragraph | `dev/bench_results/eda07788/tables-v3.md`, Protocol-v3 confirmation (withdrawn), Paired conversion attribution: 24 of 24 for `qam16-r12-normal-gap-native-vs-xdsopl`, 19 of 24 for `qam16-r12-normal-gap-portable-vs-xdsopl`, 0 of 24 for both 64-QAM rows | Every figure in that paragraph is a `-native-vs-xdsopl` row. The rewrite says so and cites the attribution block rather than repeating a count. |

## Numbers with no committed artifact

| Report | Number | Disposition |
|---|---|---|
| `dev/active/eda07788/findings.md` | the `+20` filler value an earlier analysis binary printed | Preserved as a falsification record under *Falsified, contradicting and negative results*. It is the superseded output of a corrected tool, not a current measurement, so no committed artifact carries it and none should. |

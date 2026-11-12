# Handoff: Remove artifacts and unresolvable references (daef2dc3), session 37

**Date:** 2026-11-12

## Current state

`@/issue/daef2dc3` is in progress. `cargo-ci` passed at `87df671e5`.
`code-review` failed there, the fifth failure, so work stopped as the steward
directed. `@/issue/595e2fe5`, `@/issue/506f76d4` and `@/issue/3478cb98` wait on
it; `@/issue/45ecf975` waits on those, `@/issue/1aea3958` and
`@/issue/67194d70`. Nothing has been pushed.

## What happened

- The three findings of handoff-33 are closed. Workflow JSON under `dev/active`
  and `dev/research` is refused when it carries execution-record members. The
  reference check reads path-named members of JSON, TOML and YAML files. The BCH
  oracle fixtures hold no host paths and no run costs, and `run.sh` creates
  `target/oracle`. Two runs regenerated identical fixtures under SageMath 10.10.
- Epic `@/issue/55c3e433` is done, so its directory left `dev/active`. Its
  contracts moved to `experiments/aff3ct-ldpc-gaps/contracts.md`.
- The breakdowns of three rejected `b4b4b9ee` children left `dev/active`.
  Issue text that named removed locations now names base names.

## Open findings at `87df671e5`

1. **High, REQ-02 and REQ-05.** `dev/research/permanent_wave_gpu` holds a run
   receipt and compiler-resource result tables in its README.
   `dev/research/permanent_wave_gpu/scripts/compile_arch_evidence.sh` writes the
   receipt and log into the source directory. The budget admits Markdown there
   without inspecting it.
2. **Medium, REQ-04.** Base-name citations of deleted files in that README
   pass the reference check, which needs a slash in an inline path. The receipt also names an execution log
   that no longer exists.

## Next

1. The owner decides whether another round runs.
2. If it runs, audit every Markdown file under `dev/research` and `dev/active`
   for receipts and result tables, not only the cited one. Point the generator
   at `target/`.
3. Delete citations of deleted files outright. Teach the reference check that a
   bare file name with an evidence suffix (`.log`, `.out`, `receipt`) must
   match some tracked file.

## Traps

- **Do NOT rewrite a removed path to its base name.** Earlier rounds did, and
  so did this one for `55c3e433` and three done issues. Finding 2 rejects it:
  delete the citation or point it at a file that exists.
- **Do NOT treat a finding's cited file as its whole scope.** Five rounds each
  found the same class one file further. Audit the class.
- **Do NOT start a gate run on main without the other lead's hold.** A merge
  two seconds after the start moved the tree under the run.
- **A cold `cargo-ci` exceeds the gate's 900 s timeout.** Warm the build with a
  direct run first; a warm run took 394 s.

## Open questions for the invoker

- Authorize one more full-class round, accept the remaining findings as tracked
  issues outside daef2dc3, or amend REQ-04 to name the reference forms it
  covers. Recommendation: one more round, scoped as Next 2 and 3.

## References

- [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md)

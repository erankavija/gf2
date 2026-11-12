# Handoff: Separate experiment code from the library crates (45ecf975), session 38

**Date:** 2026-11-12
**Prior handoffs:** [handoff-34.md](handoff-34.md) and its predecessors.

## Current state

- `@/issue/daef2dc3` is in progress. The owner approved one final class-wide
  round after the fifth `code-review` failure at `87df671e5`. The round is on
  branch `worktree-agent-daef2dc3-c` (worktree `agent-daef2dc3-c`), 12 commits
  ahead of main, unmerged and ungated. Its last recorded verdicts are still
  `cargo-ci` passed and `code-review` failed at `87df671e5`.
- `@/issue/4f21da96` is done: the budget wrapper is removed and both gates
  passed. `@/issue/20f5343b` is rejected as superseded.
- `@/issue/1aea3958` is done: the AFF3CT comparison code lives in
  `experiments/aff3ct-ldpc-gaps`, and both gates passed.
- `@/issue/67194d70` is ready. Its validation is the first line of tonight's
  window, from worktree `agent-67194d70` detached at `259366a0b`.
- `@/issue/3478cb98`, `@/issue/506f76d4` and `@/issue/595e2fe5` wait on
  daef2dc3. `@/issue/45ecf975` waits on them and on 67194d70.
- The ignored studies leftovers on this host are deleted; no tracked file changed.

## What happened

- The round on the branch:
  - removes receipts and result tables under `dev/research` and `dev/active`,
    measured values from session records and workflow JSON, unread tuning
    envelopes with run records, a raw API response and the last committed
    benchmark result table;
  - points the tuning-campaign driver's complete envelope at `target/`;
  - restores five HIP kernel sources an earlier round deleted.
- The branch's checkers:
  - `scripts/check-tracked-budget.py` reads Markdown, JSON and CSV contents
    under `dev/`, `experiments/`, `benchmarks/` and `crates/` for receipts, run
    records and measurement tables;
  - `scripts/check-references.py` checks `@/inv` citations, backticked bare
    file names, deleted evidence names, and removed paths in open issues.
- An independent read-only audit ran over the branch before any gate. Its
  findings drove the last four commits.
- Dated session records stay unedited and are exempt from the reference check
  by the owner's ruling of 2026-11-12.
- The validation run writes its record to the main checkout's gitignored
  `.agents/campaign-evidence/67194d70/validation`. The binary prints its
  executable digest first.

## Next

1. After tonight's window, read the validation record under
   `.agents/campaign-evidence/67194d70/validation` in the main checkout. Check
   its execution log's start record: the executable digest must equal the
   `sha256` line the window's job output printed first. Record the location,
   the digest of the log and the verdict per anchor in 67194d70 with
   `--append-description`. Commit nothing measured.
2. On the daef2dc3 branch, `check-references.py` reports about 33 faults, all
   in `.jit/issues`. Those are tracker text, and they need these edits on main:
   - Remove the 20af7e6e document link to the deleted complete envelope.
   - Rewrite every issue description, open or closed, that cites a deleted file
     by name, a moved path, or a removed invariant. Delete the citation, or point
     it at the current path or invariant. Change nothing else. A drafted set of
     449 descriptions was lost with the session. Regenerate it: list deleted
     names with `git log --all --diff-filter=D --name-only` and match them
     against descriptions. Read every open issue's diff before applying.
   - Map removed invariant ids as follows:
     - behavioral-evidence-validity and campaign-resumability become
       campaign-evidence;
     - experiment-library-separation becomes library-first-generality;
     - semantic-test-assertions becomes shared-test-contracts;
     - active-document-layout becomes document-placement.
     Decide runtime-observed-provenance by reading its old statement in history.
3. Merge main into the branch and land it with the tracker edits in one
   landing. Ask the other lead for a hold, warm the build with a direct
   `./scripts/cargo-ci.sh`, then evaluate `cargo-ci` and `code-review`. If
   `code-review` fails, stop and report: the owner allowed no further round.
4. If it passes, close daef2dc3. Then run 3478cb98: amend open issues whose
   criteria require committed receipts or removed files; about 30 are known,
   3eeb57f6 REQ-05 among them. 67194d70 REQ-01 and REQ-02 still require a
   committed receipt. Then run 506f76d4 and 595e2fe5.

## Traps

- **Do NOT close an issue whose text names a path it deleted.** Closing it
  drops the open-issue planned-path exception, and the reference check fails on
  main. It happened with 4f21da96; rewrite the text first.
- **Do NOT run the reference check on an untracked file and trust it.** It reads
  tracked files only.
- **Do NOT treat a finding's cited file as its scope.** Five reviews found the
  same class one file further. Run an independent audit of the whole tree
  before the gate.
- **A cold `cargo-ci` exceeds the gate's 900 s timeout.** Warm it with a direct
  run first.
- **Concurrent worktree builds make fast-tier tests time out.** A direct run
  failed 15 s kills in the tuning-campaign tests while three builds ran; the
  gate's run passed.
- **The AFF3CT adapter has its own lock file, which `cargo-ci` never builds.**
  After changing the experiment crate's dependencies, build the adapter with
  `--locked`.

## Open questions for the invoker

- Measured tuning profiles that code reads, kept by reasoned exception: the
  gf2-63bad95d reference profile (a gf2-algebra test), four gf2-core data
  profiles read only by their committed-profile test, and the 0a77ecfc profile
  of the AFF3CT experiment. Remove the historical ones with their test edits,
  or keep them as configuration? Recommendation: remove the ones only a test of
  their own presence reads.
- Plan and progress files under `dev/active` still contain 46 passages that tell
  workers to commit receipts. Recommendation: rewrite the plans and leave the
  progress history.
- The force-push of the rewritten history (45ecf975 REQ-04) waits for the owner.
  Its rehearsal predates the daef2dc3 removals, so it reruns after daef2dc3
  lands, before the owner decides.

## References

- [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md),
  [handoff-34.md](handoff-34.md)

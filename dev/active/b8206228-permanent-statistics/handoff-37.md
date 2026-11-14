# Handoff: Separate experiment code from the library crates (45ecf975), session 40

**Date:** 2026-11-13
**Prior handoffs:** [handoff-36.md](handoff-36.md) and its predecessors.

## Current state

- `@/issue/daef2dc3` is done. Both gates passed after the owner's one fix round.
- `@/issue/3478cb98` is done. Both gates passed.
  - 21 criteria across 16 issues were amended by the steward's rule.
  - The `parallelism-pays` gate text was amended under owner ruling C5.
  - `@/issue/67194d70` REQ-01 and REQ-02 carry the owner-approved wording.
- `@/issue/595e2fe5` is in progress. It landed on main (`e7ac181c4`) and `cargo-ci` passed. `code-review` failed on one finding: GF(2^8) dispatch rustdoc ranked kernels as "fastest". The class fix is on branch `worktree-agent-595e2fe5` (`c54b5ed4b`), approved by the other lead, and is unmerged.
- `@/issue/506f76d4` is in progress as WIP `065594e02` on branch `worktree-agent-506f76d4`. Its frozen-record edits were lost and must be redone under the steward's option 1 (see Next).
- The `@/issue/45ecf975` REQ-04 rehearsal has finished steps 1 to 5 in `~/.local/share/gf2/rewrite-rehearsal-2`. Step 6 is open.
- `@/issue/0fe85425` and `@/issue/da6200b8` are filed under 45ecf975 and are not started.
- The night window of 2026-11-13 runs from 02:00. `agent-67194d70` stays detached at `259366a0b`.

## What happened

- 3478cb98:
  - A Sonnet worker drafted the amendments, and a second scan by the lead found two more.
  - The steward checked two points: the "dataset home" of `@/issue/5758790d` is a path inside the tree, so the amended criteria read "outside the tracked tree, at the location this issue records"; and `dev/scripts/regen-asm.sh` exists.
- 595e2fe5: a Sonnet worker cut unharnessed claims in eight files. The review found the GF(2^8) "fastest" rustdoc.
- 506f76d4:
  - A Codex sol worker deleted the validator, the producing-closure scripts, the addendum schemas, `benchmark-acceptance` and their CI steps; its inventory is in its worktree history.
  - It also edited the frozen a83583e0 and dbd8787d records. The lead reverted those edits, then lost them.
- Rehearsal numbers, steps 1 to 5:

  | Measure | Value |
  |---|---|
  | Mirror size | 698 MB → 60 MB |
  | Pack size | 56.5 MiB |
  | Blobs stripped | 2,589 |
  | Refs | 626 |
  | Commits on main | 9,697 |
  | Pinned commits mapped | 14 |
  | Compressed map | 495 KB |
  | `jit validate` | passes |

## Next

1. After the window ends:
   1. Ask the other lead for a hold on main.
   2. Merge `worktree-agent-595e2fe5`, including this handoff.
   3. Rerun 595e2fe5's `cargo-ci` and `code-review`.
   4. Close it on a pass.
2. 506f76d4, under the steward's option 1, ruled 2026-11-12:
   1. Redo edits to the a83583e0 and dbd8787d records that remove only fields, entries and sections naming deleted machinery (`evidence_index`, deleted script entries, publish and validator sections).
   2. Read the full diff and confirm that no measurement parameter, seed, threshold, decision rule or stated value changes.
   3. Record in each campaign's issue the last commit that holds the as-run files and the reason for the edit.
   4. Check that the 22 driver tests pass with no legacy branch in the driver.
   5. Then gate.
3. Rehearsal step 6:
   1. On a quiet host, run `./scripts/cargo-ci.sh` in `~/.local/share/gf2/rewrite-rehearsal-2/checkout`.
   2. Record every number above and the result in 45ecf975, using `--append-description`.
   3. The real rewrite and any push wait for the owner.
4. Then 0fe85425 and da6200b8, and 45ecf975's own criteria.

## Traps

- A gate's first `cargo-ci` after a rustdoc or tracker change can exceed the 900 s timeout and report `error`. An immediate rerun on the warm tree passes.
- `git checkout -- <path>` on a worker's uncommitted worktree discards its edits. Commit the worker's state as WIP before reverting anything.
- "Dataset home" is a path in the tree (`permanent-zero-fraction/<campaign-id>/`, issue 5758790d), not an outside location.
- The traps of handoff-36 still hold.

## Open questions for the invoker

- The measured tuning profiles kept by reasoned exception (open since handoff-35).

## References

- [handoff-36.md](handoff-36.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md)

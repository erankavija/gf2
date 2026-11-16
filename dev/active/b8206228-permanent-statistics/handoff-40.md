# Handoff: Separate experiment code from the library crates (45ecf975), session 43

**Date:** 2026-11-15
**Lead:** gf2-00, successor of gf2-e9. Steward: gf2-96.
**Prior handoffs:** retired under the owner's 2026-11-14 rule. This file is the directory's current handoff. `handoff-39.md` stays because `@/issue/d1c0e062` records it as the handoff that carries the retired files' traps. Its `## Traps` section still binds, and the traps below add to it.

**Wind-down (owner, 01:22):** every stream pauses except the tracker carve-out (`@/issue/6ba9be17`, lead gf2-e3), the history rewrite included. This stream runs nothing until gf2-e3 calls gf2-00 to implement `@/issue/2a4c2684`.

Artifacts outside the tree, where `<gf2-share>` is the directory the history-rewrite runbook backs up to:
- `<gf2-share>/45ecf975-handoff-38/`: review verdicts, worker briefs, the d1c0e062 trap ledgers (`d1c0e062-traps-r3.tsv`, `d1c0e062-traps-codex-r3.txt`), the owner reword candidates.
- `<gf2-share>/rewrite-plan/2026-11-14b/`:
  - `branches-classified.tsv` with `classify.py`, `no-issue.tsv`, `worktrees.tsv`, `dispositions.tsv` (step 2);
  - `reword-noscope.md`, `reword-set.tsv` (rewords);
  - `2a4c2684-design.md`, the 2a4c2684 design agreed with gf2-e3.
- `<gf2-share>/rewrite-rehearsal-4/`: the rehearsal script `rehearse.sh`, its log, scratch `reword_subjects.py`, the mirror and the rewritten checkout.

## Current state

| Issue | State | Branch head | Where it stands |
|---|---|---|---|
| `@/issue/d1c0e062` | done, on main `a383d8a6c` | — | cargo-ci passed at `8faf09b8d`; code-review passed on attempt 3 at `38123e746`. Frees 325,042 bytes. Worktree `target/` removed. |
| `@/issue/506f76d4` | parked | `worktree-agent-506f76d4` `b65c028cd` | Rework 2 is done and reviewed by the lead. The forced cargo-ci gate at `38da2e2d5` failed: the `test` step exited 100 after 39 s, and the record does not name the test; a solo run of that step passed at 00:17. Missing: a cargo-ci pass, then its third code-review. A failure goes to the steward. It lands only when no window builds `experiments/aff3ct-ldpc-gaps/`. |
| `@/issue/66fefd9e` | parked | `worktree-agent-66fefd9e` `e0c9b7b16` | Rework 2 is not gated. Its record names 20 files that other leads retired, so check-references fails on the branch. The fix is a record edit that the session's classifier refuses unless the owner approves it. Missing: that edit, then gates. |
| `@/issue/da6200b8` | parked | `worktree-agent-da6200b8` `c33d20e56` | Rework 2 is not gated, and its base predates `a33af036b`. Missing: a main merge, then gates after 506f76d4. A third failure goes to the owner, with option B. |
| `@/issue/0fe85425` | parked | `worktree-agent-0fe85425` `b1eb09c54` | Ungated, base predates `a33af036b`. Missing: the rework that `handoff-39.md` Next 5 listed, retained under Next here, after 506f76d4. |
| `@/issue/67194d70` | parked, on main | — | REQ-01 recorded. REQ-02 and REQ-03 are covered by `experiments/permanent-statistics/tests/campaign_validation.rs`. Missing: its gates, on main under a hold. |
| `@/issue/2a4c2684` | backlog, task of 6ba9be17 | — | Waits for 3fb4accb and 6310679b. gf2-00 implements it on gf2-e3's call. |
| `@/issue/5f33fb28`, `@/issue/95077f84`, `@/issue/411e7267` | backlog, stopped | — | After the rewrite. |

## Owner decisions and steward rulings that bind the successor

- **Wind-down (2026-11-15 01:22):** only the carve-out runs. The history rewrite, its rehearsal rerun, the reword and branch lists and any tracked runbook update pause. The benchmark window of 2026-11-15 was cancelled.
- **Rewrite order:** the tracker cutover (`@/issue/6889e6e6`) runs inside the rewrite's quiet window before the extraction step, so the rewrite drops `.jit/**` from history. 45ecf975 depends on 6889e6e6. The rewrite and any push wait for the owner's word, and nothing is pushed.
- **Parked branches (owner, 2026-11-15, via steward gf2-96):** carried through the rewrite as rewritten refs. The runbook's precondition becomes "merged, abandoned, or on the parked list" in the tracked runbook update made with 2a4c2684.
- **Step-2 dispositions so far:**
  - Abandon: `worktree-agent-2bca2b28`, `wip-3461a52a-cb96b9810`, `jit-259acf37-direct-plane`, `jit-befccab3-toom3`.
  - Parked: `worktree-agent-243d1160`, `worktree-agent-7fbd1042`, `worktree-agent-4443de83`, and `worktree-agent-9a1b24fc-a`.
  - Untracked arms kept through reseating: `.agents/worktrees/agent-de311215` and `.agents/worktrees/agent-fd9d5416`.
  - With the owner: the 17 no-issue branches, with the recommendation to delete all (`origin` holds only `main` and `badges`).
- **Rewords:** a commit that implements or serves an issue carries `jit:<id>`; a free scope conforms otherwise.
  - Approved: the owner's 36 in `reword-candidates-owner.tsv`, `0012635e024e`, `02d6abb01c05`, `8610e44ccfea` → `docs(jit:c62356d2): …` and `fb93d535c4ac` → `chore(jit:b4b4b9ee): …`.
  - Pending the owner: `a33af036b983` → `chore(jit:2caf738d): …`, and the five ambiguous `42e42101a94d`, `797e1d5205af`, `79f9117d417b`, `f57fbec1e3d4`, `fceb1c842c57`.
  - The other eleven free scopes stay.
- **2a4c2684 (steward and gf2-e3):** remap pins in a tracker clone mounted in the rewritten checkout, validate there, then fast-forward the primary tracker. The commit map stays in the code repository only. gf2-e3's three additions are in the design file.
- **Gates:** merge main before any review or plan gate; a verdict on a base that predates `a33af036b` is rerun. A cargo-ci timeout under load is rerun and counts as no verdict. Cargo-ci slots now belong to gf2-e3.
- Standing: da6200b8 option A and option B on a third failure; measured tuning profiles: tests that read them are removed by default; no crate under `crates/` depends on `dev/tools` or `experiments/`; gate before merge; count commit subjects before every merge.

## History rewrite (45ecf975 REQ-04)

Rehearsal 4, run from main `ff0f2259a` and `origin/main` `b68598fc6`:
- bundle verify ok; refs 656 before and after; pack 280.17 MiB → 57.23 MiB; `main` 9958 → 9928 commits; 2879 strip ids.
- 14 pinned commits mapped, none pruned; commit map 500 KB gzipped.
- 0 nonconforming subjects left with the scratch reword set; `jit validate` passed.
- Step 7 cargo-ci hit only load timeouts, at load 50–87. The rerun is open and paused.
- The rewritten tree was 67 kB under `max_tree_bytes`; d1c0e062 has since freed 325 kB, and the cutover frees `.jit`.

The combined rehearsal (`@/issue/6b9e6b14`) reuses `<gf2-share>/rewrite-rehearsal-4/rehearse.sh` with the 2a4c2684 changes, run on gf2-e3's scratch cutover result.

## Next

1. On gf2-e3's call, once 3fb4accb and 6310679b land: implement 2a4c2684 per the design file, as one tracked change to the runbook, `rewrite_paths.py`, `map_pins.py` and the precondition. Then run the combined rehearsal 6b9e6b14 with gf2-e3.
2. When the owner lifts the wind-down:
   - 506f76d4: gate and code-review;
   - 66fefd9e: the record edit, with the owner's approval in the session, then gates;
   - da6200b8 and 0fe85425: a main merge, then gates (0fe85425's rework: drop its edits to files 506f76d4 deletes, drop its `grand_phase1_smoke.rs` hunk, remove gf2-coding's dev-dependency on tuning-campaign-support, get the peer reviews of its `scripts/cargo-ci.sh` steps and `experiments/` files);
   - 67194d70: gates on main;
   - rehearsal 4: the step-7 rerun;
   - the tracked reword-set update once the owner answers.

## Traps

- Landing by plumbing: after `git update-ref` moves `main`, sync the main checkout's index and working tree for the changed paths only. Write `.jit/events.jsonl` as the new blob plus the working-tree lines absent from the old commit, by set and not by prefix: peers' tools reorder lines. The session's classifier refuses the sync unless the owner approves it; if it does, move `main` back so the ref, index and working tree agree, then ask.
- A worktree whose base predates `c34e8203` lacks `scripts/check-commit-subject.py`, which the shared commit-msg hook calls; there, commit with `git commit-tree` after running the budget check and counting the subject.
- `jit gate evaluate` reuses a verdict when the gate's declared inputs are unchanged. After an environment failure, rerun with `--force`.
- Merging main into a branch: take the branch's own `.jit/issues/<its-id>.json` (its gate records and newer record), not main's.
- A trap ledger that drops traps by topic ("campaign detail") fails REQ-02-style criteria. Re-judge each drop on the criterion's own ground, and have the other vendor rerun the search.
- The session's classifier refuses `jit issue update --description-file` unless the owner approves it in that session; `--append-description` passes.

## References

- [handoff-39.md](handoff-39.md) (traps), [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md), [reword_subjects.py](45ecf975/reword_subjects.py)

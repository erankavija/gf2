# Handoff: Separate experiment code from the library crates (45ecf975), session 44, rewrite line

**Date:** 2026-11-17
**Lead:** gf2-8a under steward gf2-92.
**Prior handoffs:** [handoff-40.md](handoff-40.md) (parked branches, binding decisions), [handoff-39.md](handoff-39.md) (traps).

## Current state

- The history rewrite of `@/issue/45ecf975` REQ-04 ran on 2026-11-17 between 01:34 and 01:53 local, on the owner's go typed in this session. The steward force-pushed `main` at about 02:00 on the owner's word and pushed the tracker to its home.
  - `main` had 10317 commits before the rewrite and 7949 after it; its rewritten tip is `a705332ea`. Then came `b4eb2b35e`, the mechanical old-ID pass of step 11.
  - The pack shrank from 280 MiB to 50.17 MiB.
  - The repository went from 82 refs to 77, and every ref is free of `.jit` paths.
- Landed this session:
  - `@/issue/2a4c2684` (merge `c2ef25ffa`), the separate tracker through the rewrite;
  - `@/issue/23c47bea` (merge `a705332ea`), step 7 on the kept branch set.
- The step-11 worker pass over 163 references ran on branch `worktree-agent-45ecf975-oldids`, together with the runbook correction `972bd3f03` from the real run. The commits of its landing state its outcome, and the Notes of `@/issue/45ecf975` hold its row counts.
- Parked, carried through the rewrite as rewritten refs. Every gate reruns on the head that lands; their recorded verdicts name pre-rewrite commits.
  - `@/issue/84022ee3`: REQ-02 tests, code-review passed. Two cargo-ci runs gave no verdict: a 900 s timeout and the 60 s suite budget.
  - `@/issue/506f76d4`: main merged before the rewrite, Claude review passed, cargo-ci and its third code-review still missing. It lands only when no window builds `experiments/aff3ct-ldpc-gaps/`.
  - `@/issue/66fefd9e`: main merged, no record edit needed (steward accepted), gates missing.
  - `@/issue/da6200b8`: lands before `@/issue/83ca8ca5` (steward ruling). It conflicts with main in `scripts/check-references.py` and `scripts/reference-exceptions.toml`.
  - `@/issue/0fe85425`: waits on `@/issue/18276ff1`, which is parked.
  - `@/issue/67194d70`: its gates run on main.

## Rollback material

`<gf2-share>` is the gf2 data directory of the host, the history-rewrite runbook's backup location.

`<gf2-share>/pre-rewrite/` holds what a rollback needs. It stays until the owner says otherwise:
- the code repository's mirror `gf2.git`, with its all-refs bundle `gf2.bundle` (verified);
- the tracker's mirror `tracker.git`, with its bundle `tracker.bundle` (verified);
- `git-aside`, the primary checkout's pre-rewrite `.git`, and `git-jit`, the pre-rewrite claims;
- the worktree list with HEADs, and `refs-before.txt`;
- `worktree-ignored/`, the ignored non-cache content of the 26 removed worktrees, archived before their removal;
- the step-2 branch decisions, `step2.tsv`;
- the commit map `commit-map-a4517496b58ccc753b9ad7cf8f5c80cb0af2ffa6a4daf2918038fe228db87d09.gz`, which is archived in neither repository by the owner's decision;
- the old-ID inventories.

The run log, the scripts of steps 3 to 7 and 10 to 11, and the scratch rewritten checkout are in `<gf2-share>/rewrite-2026-11-17/`. Rehearsal 5 is in `<gf2-share>/rewrite-rehearsal-5/`; it may be deleted.

## What happened

- `@/issue/2a4c2684`: one tracked change to the runbook and the rewrite tools, after five reworks.
  - Its record keeps, in its section "Falsified design statements", the design statements the rehearsals contradicted, each beside the contradicting result.
  - Owner decisions changed it twice: the commit map lives in neither repository, and old IDs are fixed after the reseat.
- Rehearsal 5 ran nine times, each run finding a defect in the runbook or in its own script. The run of 01:08 to 01:20 on the jit binary of 00:35 was clean, cargo-ci in the rewritten checkout included.
- The real run differed from the rehearsal in two ways, corrected in the runbook:
  - The reseat clones with `--mirror`, because a bare clone drops `refs/archive/`, `refs/salvage/`, `refs/backup/` and `refs/claude/`.
  - A clone carries neither `.git/info/exclude` nor the local `branch.*` and `core.logallrefupdates` settings, so they were restored from the moved-aside `.git`.
- Step 9 pushed `main` alone. `badges` is an untouched orphan, `origin` holds no tags, and the local tags are backups.
- Two tracker records named paths that only to-be-deleted branches held. The steward reworded them before the run. `@/issue/66fefd9e` and `@/issue/506f76d4` still name paths held only by their own parked branches, so `scripts/check-references.py` passes only while those branches exist.
- Filed during the session:
  - `@/issue/23c47bea`, done;
  - `@/issue/91086388`, a flaky `l2` check in `scripts/cargo-ci-failure.test.sh`.

## Dispatch record

| Work | Vendor, model | Result |
|---|---|---|
| 84022ee3 parse tests | Codex gpt-6-luna xhigh | accepted |
| 2a4c2684 implementation | Claude Opus 5.5 | accepted after lead and review rework |
| 506f76d4 merge conflict | Codex gpt-6.1-sol high | accepted |
| Review of 84022ee3 and 506f76d4 merge | Claude Sonnet 5.5 | PASS, PASS |
| 2a4c2684 rework 1 (pruned pins) | Claude Sonnet 5.5 | accepted |
| 2a4c2684 rework 2 (map outside the trees, old_ids.py) | Codex gpt-6.1-sol high | accepted |
| Review of 2a4c2684 branch | Claude Opus 5.5 | FAIL, 3 blocking, fixed |
| 2a4c2684 rework 3 (digest-covered records) | Claude Opus 5.5 | accepted |
| 2a4c2684 rework 4 (worktree names, check) | Codex gpt-6-luna xhigh | accepted |
| 2a4c2684 rework 5 (short IDs) | Claude Sonnet 5.5 | accepted |
| 23c47bea | Codex gpt-6-luna xhigh | accepted |
| Step-11 swarm A, C | Codex gpt-6-luna xhigh | see the landing |
| Step-11 swarm B, D | Claude Haiku 5.5 | see the landing |

Code-review gates ran on Codex gpt-6.1-sol (the registry's reviewer).

## Cutover observations

- jit keeps claims, leases and the worktree identity in the code repository's `.git/jit/`, not in the tracker (`@/issue/15f56b64`).
- `scripts/check-references.py` checks tracker prose only in the primary checkout. A linked worktree skips the records, so prose written from a worktree escapes the check.
- The checker accepts a retired path when a local branch head holds it. Removing a branch can therefore break `main`'s reference check.
- A gate killed at its timeout left `cargo-pool-run` clients holding pool tokens. jit `074757072` sends SIGTERM to the checker's process group first.
- The 900 s cargo-ci timeout was shorter than a cold-worktree cargo-ci, which took up to 1172 s. The timeout is now 1800 s.
- `code-review` declares no inputs, so it counts only for its recorded commit. The merged-tree step judges cargo-ci only (steward ruling). A proposal for declared inputs went to the owner.
- A last-gate pass completes an issue only from `gated`. Request done before the landed gate, so the plan's F-28 is falsified in its unconditional form.
- With machine-local gate-run records, a fresh tracker clone validates without copied run files.

## Next

1. Delete `<gf2-share>/rewrite-rehearsal-5/` when convenient. Keep `pre-rewrite/` until the owner releases it.
2. Resume the parked branches on the owner's word, each with a main merge and every gate at the head that lands.
3. `@/issue/45ecf975`: close its remaining children, then the epic.

## Traps

- Do not run `git fetch` from an unrewritten remote or clone into the reseated repository: it brings the old history back.
- A relayed owner approval does not authorize ref writes; the owner types it in the operating lead's session.
- Rehearsal scripts under `set -euo pipefail` fail silently on a `grep` that matches nothing. Wrap such greps with `|| true`.
- A hand-kept branch list goes stale within hours. Derive it from the refs and the tracker states at the moment of use.

## References

- [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md), [plan.md](45ecf975/plan.md), [handoff-40.md](handoff-40.md).

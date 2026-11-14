# Handoff: Separate experiment code from the library crates (45ecf975), session 43

**Date:** 2026-11-15
**Lead:** gf2-00, successor of gf2-e9. Steward: gf2-96.
**Prior handoffs:** retired under the owner's 2026-11-14 rule. This file supersedes `handoff-39.md`. The `@/issue/d1c0e062` branch edits that file's traps, so it stays on `main` until that branch lands. The landing folds its trap section into this file and deletes it.

Work stops at 01:40 local for the 2026-11-15 02:00 benchmark window. Nothing of this stream runs during the window.

Artifacts outside the tree:
- `<gf2-share>/45ecf975-handoff-38/`: review verdicts, worker briefs (now including `brief-506f76d4-rw2.md`), 66fefd9e inventories, reword candidates.
- `<gf2-share>/rewrite-plan/2026-11-14b/`: step-2 branch classification (`branches-classified.tsv`, `classify.py`), `no-issue.tsv`, `worktrees.tsv`, the free-scope reword table `reword-noscope.md`, and the approved reword set `reword-set.tsv` (41 entries).
- `<gf2-share>/rewrite-rehearsal-4/`: rehearsal 4 script, log, scratch `reword_subjects.py`, mirror and rewritten checkout.

`<gf2-share>` is the directory the history-rewrite runbook backs up to.

## Current state

Every issue below is assigned to `agent:gf2-00` (main `747cae404`). Each branch is committed and its worktree clean.

| Issue | State | Branch head | Gates | Where it stands |
|---|---|---|---|---|
| `@/issue/d1c0e062` | in progress | `worktree-agent-d1c0e062` | cargo-ci passed at `5fde7f680`; code-review failed twice | Round 1: prompt rule and a progress.json pointer, both fixed. Round 2: an uncarried trap. The prior trap ledger dropped 169 traps, 67 as campaign detail. Attempt 3 re-judges every drop; its commit, if the worker made one, is the branch head. A third failure goes to the steward. Frees about 340 kB. |
| `@/issue/506f76d4` | in progress | `worktree-agent-506f76d4` `01d9c0920` | cargo-ci failed on load only (the `test` step passes solo); code-review not rerun | Rework 2 (protocol-version guard, `FROZEN_PROTOCOL_VERSION`, two tests) is done and reviewed by the lead. It touches `experiments/aff3ct-ldpc-gaps/` and `crates/gf2-core`, so it waits for the window's end. A third code-review failure goes to the steward. |
| `@/issue/66fefd9e` | in progress | `worktree-agent-66fefd9e` `2d3dc335a` | rework 2 not gated | Main merged in. Its record names 20 files that other leads retired, so check-references fails on the branch. The fix is a record edit after d1c0e062 lands, and it needs the owner's approval in the session. |
| `@/issue/da6200b8` | in progress | `worktree-agent-da6200b8` `c33d20e56` | rework 2 not gated | Base predates `49c78483d`: merge main, then gate after 506f76d4. A third failure goes to the owner, with option B. |
| `@/issue/0fe85425` | in progress | `worktree-agent-0fe85425` `04ab2400b` | ungated | Base predates `49c78483d`. Rework after 506f76d4 lands, as `handoff-39.md` Next 5 states. |
| `@/issue/67194d70` | in progress, on main | — | cargo-ci, code-review never run | REQ-01 recorded. REQ-02 and REQ-03 are covered by the tests in `experiments/permanent-statistics/tests/campaign_validation.rs`; the gates still have to run, on main under a hold. |
| `@/issue/2a4c2684` | backlog, waits for 6310679b | — | — | Design drafted outside the tree (`<gf2-share>/rewrite-plan/2026-11-14b/2a4c2684-design.md`). No tracked edit before 6310679b lands. |
| `@/issue/5f33fb28`, `@/issue/95077f84`, `@/issue/411e7267` | backlog | — | — | After the rewrite. |

## Owner decisions and steward rulings of 2026-11-14/15 that bind the successor

- **Priority (owner, via steward):** the history rewrite and the `.jit` carve-out (`@/issue/6ba9be17`) lead the project. Order: rewrite readiness, then land d1c0e062 and 506f76d4, then 66fefd9e, da6200b8, 67194d70, 0fe85425. Every live branch is merged or abandoned before the quiet window; no new long-lived branch. 5f33fb28, 95077f84 and 411e7267 wait until after the rewrite.
- **fe2221d5 is rejected:** the tracker split removes `.jit/` from the tree.
- **Tracker split and the rewrite:** the tracker cuts over to its own local-only repository inside the rewrite's quiet window, before the extraction step, so the rewrite drops the tracker paths from history. The task `rewrite-runbook-tracker` arrives when the 6ba9be17 plan lands: the path filter drops `.jit/**`, map_pins reads issues through the tracker data root, the rehearsal mounts the tracker before `jit validate`, the backup bundles the tracker, reseating keeps it. Until the plan lands, make no tracked edit of the runbook, `rewrite_paths.py`, `map_pins.py` or `reword_subjects.py` that concerns the tracker.
- **Rewords:** a commit that implements or serves an issue carries `jit:<id>`; a free scope conforms when the commit served no issue. Approved set: the owner's 36 in `reword-candidates-owner.tsv`, the two over-length fixes `0012635e024e` and `02d6abb01c05`, `5855c6dabc90` → `docs(jit:c62356d2): …`, `fb93d535c4ac` → `chore(jit:b4b4b9ee): …`, and `49c78483d964` → `chore(jit:2caf738d): remove obsolete repository JIT skill copies` pending the owner's confirmation. Five ambiguous subjects (`42e42101a94d` and the four tech-debt triage commits) are with the owner. The other eleven free scopes stay. The reword-set update of `reword_subjects.py` lands on its own branch once the owner answers.
- **Step 2:** the 17 no-issue branches go to the owner with the recommendation to delete all; `origin` holds only `main` and `badges`, so the runbook's step-9 deletion of two remote branches is stale and is fixed in the runbook update. `worktree-agent-ae312a8f6f84377dd` is the steward's live plan branch. `backup-506f76d4-e60912a40` is deleted with step 2 after 506f76d4 lands.
- **Gates:** merge main into a branch before any review or plan gate; a verdict produced on a base that predates `49c78483d` (removal of the repository skill copies) is rerun. A cargo-ci timeout under load is rerun and is no failure against the issue or the rehearsal.
- **Pushes:** all held until the rewrite. The rewrite itself and any push wait for the owner's word.
- **2a4c2684 decisions (steward, 2026-11-15):** remap pins in a tracker clone mounted in the rewritten checkout, validate there in step 7, then fast-forward the primary tracker; the commit map stays in the code repository only. Record both in the issue when it becomes ready.
- **506f76d4:** its next code-review is the third; a failure goes to the steward before any rework.
- Standing rulings of `handoff-39.md` that still hold: da6200b8 option A (positive list) and its last rework, with option B on a third failure; 66fefd9e retired rows drop out; measured tuning profiles: tests that read them are removed by default; no crate under `crates/` depends on `dev/tools` or `experiments/`; gate before merge; count commit subjects before every merge.

## History rewrite (45ecf975 REQ-04)

Rehearsal 4 from main `ff0f2259a`, `origin/main` `b68598fc6`, 2026-11-14 23:09:
- bundle verify ok; refs 656 before and after; mirror 769 MB → 61 MB; pack 280.17 MiB → 57.23 MiB; `main` 9958 → 9928 commits (30 emptied by the path filter).
- strip-ids 2879; filter-repo 32 s; map_pins mapped 14 pinned commits, none pruned; commit map 500 KB gzipped.
- rewritten tree with the map: 2932 files, 33,972,401 bytes, 67 kB under `max_tree_bytes` until d1c0e062 and 506f76d4 land.
- nonconforming subjects left in the mapped `origin/main..main`: 0 with the scratch reword set.
- `jit validate` passed. Step 7 cargo-ci: timeouts only under load 50–87 (3620 s); rerun warm on a quieter host after the window, in a slot agreed with gf2-7d (about two cargo-ci runs host-wide fit the 900 s gate).

## Next

1. After `window end` appears in `.agents/bench-window/window.log`: read the d1c0e062 attempt-3 result and its ledger `<gf2-share>/45ecf975-handoff-38/d1c0e062-traps-r3.tsv`. Have Codex rerun the trap search over the retired files independently, and reconcile the two inventories. Each trap is carried (cite where) or dropped on the criterion's own ground (a later handoff records it resolved). Then run code-review, its third and last; a failure goes to the steward with the report. Land it on a pass. Fold the branch's `handoff-39.md` traps into this file, delete `handoff-39.md`, and move its `jit doc` link. Remove the worktree's `target/` and report `df -h /`.
2. 506f76d4: run `jit gate evaluate 506f76d4 cargo-ci --force`, because a reused verdict repeats the load failure. Then run code-review, land it, and unblock da6200b8 and 0fe85425.
3. 66fefd9e: get the owner's approval for the record edit, drop the 20 retired names, then gate and land.
4. da6200b8 and 0fe85425: merge main, then gate. 67194d70: run its gates on main under a hold.
5. Rerun step 7 of rehearsal 4 (`<gf2-share>/rewrite-rehearsal-4/checkout`, `BUSY_HOST_OVERRIDE=1 ./scripts/cargo-ci.sh`). After the owner answers on the five ambiguous rewords, land the reword-set update of `reword_subjects.py` on its own short branch.
6. 2a4c2684 once 6310679b lands, then the combined rehearsal 6b9e6b14 with gf2-7d.

## Traps

- A cold or warm `cargo-ci` under load above ~40 hits the 60 s per-test kill on trivial tests (`gf2-algebra` permanent tests first). Read every `TIMEOUT` as load until a solo rerun fails; a direct warm run took 52–62 min at load 50–87.
- Merging main into a branch: take the branch's own `.jit/issues/<its-id>.json` (it holds the branch's gate records and newer description), not main's. A worker told to take `--theirs` for `.jit/` lost 506f76d4's gate records and reintroduced a deleted file name.
- A branch whose base predates `49c78483d` runs stale `.agents/skills/` helpers in its gates.
- The session's permission classifier refuses `jit issue update --description-file` unless the owner approves it in that session; ask the owner before an issue-record edit.
- `scripts/check-references.py` faults on issue text that names a file another lead retired; after any retirement on main, recheck open issues' records (66fefd9e carries 20 such names).
- `jit gate evaluate` and `jit issue update` in a worktree record on that branch's `.jit`. After merging a gated branch, copy its `.jit/gate-runs/<issue-uuid>/` records into main's ignored `.jit/gate-runs/`.
- docs-mechanical needs main's `.jit/gate-runs/` in a worktree: copy it into that worktree's ignored `.jit/gate-runs/` first.
- Main's working tree holds other leads' uncommitted tracker lines. Stage only your own paths and event lines, never check out or reset `.jit/events.jsonl`, and merge with plumbing (`git merge-tree`, `git commit-tree`, `git update-ref`). Plumbing skips the pre-commit hook, so run `scripts/check-tracked-budget.py --range` on the new commit first.
- Run `scripts/check-references.py` after every `jit issue update` and before committing tracker state. A bare file name in issue prose faults.
- The dispatch worktree script refuses while main has uncommitted tracker changes. Create worktrees with `git worktree add` from main.
- A gate result is evidence only through `jit gate status <id> <gate>`; read findings with `--findings`.
- A mechanical prose pass can change a runnable command or a criterion's meaning. Check code blocks and `## Success Criteria` lines after one.
- Sweeps by one agent miss wording forms. Slice by directory and reconcile two independent inventories by different vendors.
- Commit subjects from `wip(...)` commits and default merge messages reach main. Squash or reword on the branch, and count every subject with `${#s}`. A `git commit -F <file>` reuses whatever the file holds: rewrite the message file before each commit.
- Deleting machinery that validates committed artifacts can take live run-time checks with it. List what the deleted code checked on fresh results before deleting it.
- `git stash` is shared across worktrees and sessions. Never use it.
- `rm -rf` outside the scratchpad is refused unless the owner approves it in that session.
- `breakdown.json` stays excluded from audit by recorded owner decision (`240b7618`, `679cf170`). Do not edit or resync it.
- Concurrent builds make fast-tier tests miss their kill, and GPU-device tests fail under contention. Rerun solo before reading a failure as a regression.
- Stage explicit paths and check `git diff --cached --stat`; `jit` materializes shared state into the checkout that runs it.
- `jit doc add` is idempotent per path: run `jit doc remove` on the old link first. Before deleting a file, remove its `jit doc` links and asset entries, then run `jit validate`.
- Reference checks read tracked files only.
- Do not rewrite a removed path to a bare base name. Point the citation at an existing file or follow the name with "(removed by @/issue/<id>)".
- The history rewrite needs `map_pins.py`. Nothing is pushed before the owner's go.
- `codex exec` workers cannot write `.git`; start a background `codex exec` with `< /dev/null`.
- Do not `git checkout --` or `git restore` a worker's uncommitted edits. Commit them as WIP first.
- A commit that changes public rustdoc needs `cargo doc`.
- Passing gates are stale once shared code changes. Re-gate.
- The AFF3CT adapter has its own `Cargo.lock`; after a dependency change, build it with `--locked`.
- Do not edit `scripts/cargo-ci.sh` while an instance runs.
- A falsification must reinstate the real pre-fix code byte-for-byte.
- Kill only PIDs from your own process tree. zsh's `status` is read-only. `/usr/bin/time` is not installed.

## References

- [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md), [reword_subjects.py](45ecf975/reword_subjects.py)

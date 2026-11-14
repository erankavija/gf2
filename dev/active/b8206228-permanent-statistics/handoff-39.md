# Handoff: Separate experiment code from the library crates (45ecf975), session 42

**Date:** 2026-11-14
**Lead:** gf2-e9, successor of gf2-a6. Steward: gf2-42.
**Prior handoffs:** retired under the owner's 2026-11-14 rule. This file is the directory's single current handoff, and it carries the live traps of the retired ones.

The work wound down at the owner's order. Nothing new was dispatched after it, and no gate step was started after the second wind-down message.

Artifacts outside the tree are in `<gf2-share>/45ecf975-handoff-38/`, where `<gf2-share>` is the directory the history-rewrite runbook backs up to. They include:
- review verdicts: `506f76d4-review1.txt`, `506f76d4-review2.txt`, `da6200b8-review1.txt`, `da6200b8-review2.txt`;
- the worker briefs `brief-*.md`;
- the two 66fefd9e inventories (`claude-…`, `codex-…`) and the reconciled section;
- the d1c0e062 trap ledger;
- `dispatch-log.md`, `load-notes.txt`;
- `reword-candidates-owner.tsv`.

## Current state

Every issue below is assigned to `agent:gf2-e9`; the successor reassigns them. Each branch is committed and its worktree clean. "Gated" means gated on the branch.

| Issue | State | Branch head | Gates | Where it stands |
|---|---|---|---|---|
| `@/issue/67194d70` | in progress, on main | — | cargo-ci, code-review never run | REQ-01 is recorded in the issue: the validation passed, 10 of 10 anchors. REQ-02 and REQ-03 still need the gates. |
| `@/issue/506f76d4` | in progress | `worktree-agent-506f76d4` `ea5b56748` | cargo-ci passed at `88e04e1a4`; code-review failed twice | Second failure: version-1 cold cells. See Next. |
| `@/issue/d1c0e062` | in progress | `worktree-agent-d1c0e062` `9e2bfff58` | not started | Retirement complete. check-references, docs-mechanical and `jit validate` pass. It frees 337,199 bytes against main `eacbb3a91`. |
| `@/issue/66fefd9e` | in progress, on main with two failed reviews | `worktree-agent-66fefd9e` `8320233b8`, base `96fc9b751` | rework 2 not gated; one cold cargo-ci timeout | Rework 2: 4 passages rewritten and both inventories reconciled in the issue. A third review failure goes to the owner. |
| `@/issue/da6200b8` | in progress, on main with two failed reviews | `worktree-agent-da6200b8` `c33d20e56` | rework 2 not gated | Option A allow-list at `be55de2b9`. A third failure goes to the owner, with option B on the table. |
| `@/issue/0fe85425` | in progress | `worktree-agent-0fe85425` `04ab2400b` | ungated | Rework not dispatched. See Next. |
| `@/issue/5f33fb28` | backlog, blocked by 506f76d4 | — | — | Owner decisions recorded in the issue. |
| `@/issue/fe2221d5` | backlog, blocked by 506f76d4 | — | — | Interim stopgap, low priority. |
| `@/issue/95077f84`, `@/issue/411e7267` | backlog / ready | — | — | Untouched. |

## Owner decisions and steward rulings of 2026-11-14 that bind the successor

- **5f33fb28:**
  - The `@/issue/dbd8787d` declaration is retired. The nine `tuning-extent-campaign-driver.rs` tests that read the `@/issue/a83583e0` owner through it go with it.
  - The `experiments/aff3ct-ldpc-gaps/` tuned arm keeps its included coding profile. The REQ-02 reason, recorded in the issue: it is experiment code outside the library crates, and a tuned arm is gf2's counterpart to the reference's fastest configuration under `@/inv/performance-deliverable`.
- **History rewrite rewords:** the Codex review's 11 proposals replace the drafts.
  - Ten apply as proposed.
  - `4fe5b6bf8162` exceeded 71 characters and becomes `docs(jit:daef2dc3): write shortened paths in full in BCH investigation` (70).
  - All are in `reword-candidates-owner.tsv`. That file, not `reword-candidates.tsv`, feeds the next rehearsal.
- **da6200b8, option A:**
  - The written-name rule is a short positive list of plain forms, stated once in the docstring. Everything outside it excuses nothing.
  - Rework 2 met the terms: 0 new `writer_helpers`, 3 reasoned exceptions, `scripts/check-references.py` net −165 lines.
  - It was the last rework. A third code-review failure goes to the owner with option B: close on the round-1 fixes under `@/inv/caller-trusted-fast-paths`. Before gating, merge main after 506f76d4 lands and recheck the `dev/tools/tuning-campaign-support/config/protocol.md` exception, whose `freeze` writer 506f76d4 may remove.
- **66fefd9e retired rows:** inventory rows and history entries in files retired by `@/issue/d1c0e062` are marked "retired by owner decision" and drop out.
  - Rework 2 rewrites 4 passages, 68 in total.
  - The branch no longer touches `dispatch-7a816262-phase1.md` (removed by @/issue/d1c0e062) and must not bring it back on merge.
  - Line numbers in the inventory are at `96fc9b751`.
- **History retirement:** old history files are retired, not edited.
  - This directory keeps one current handoff.
  - This handoff deletes `handoff-38.md` and carries the traps the d1c0e062 branch condensed into it.
  - The `@/issue/d1c0e062` branch still edits that file. Its merge resolves the file as deleted, and its change to 45ecf975's doc links keeps this handoff's link.
- **fe2221d5:** an interim stopgap. The tracker moves to a separate repository mounted at `.jit/` and ignored by the code repository; a planning lead under the steward designs that move. Keep fe2221d5 behind 66fefd9e, d1c0e062, da6200b8 and 5f33fb28 until the steward says whether the plan's timing makes it unnecessary. The `.jit/events.jsonl` exception stays. Do not change the rewrite runbook for the tracker split yet.
- **cargo-ci cap:** at most two concurrent cargo-ci runs host-wide until gf2-5b's `@/issue/574ed420` (sccache jobserver fix) lands.
  - The slots belong to gf2-5b and gf2-d7 now.
  - A lead borrows by message for one named run and reports when it ends.
  - A cold-tree timeout under load is rerun, not counted, with the load recorded.
- **Disk:**
  - When an issue closes, its lead removes that worktree's `target/` in the same wave and reports `df -h /` in the landing report.
  - Below 40G free, stop builds and tell the steward.
  - Results between about 15:00 local and the clearing on 2026-11-14 are suspect as out-of-space (ENOSPC) failures and are rerun.
- **b5782224** (gf2-5b's stream) tracks the intermittent Rician `SimulationRunner` test. A scalar-lane failure of `fading::channel_model_tests::test_qpsk_rician_through_simulation_runner` is that issue, not a regression of the branch under test.
- **Standing rulings of earlier sessions:**
  - da6200b8 design C1a to C4a.
  - The 66fefd9e one-time exception.
  - Measured tuning profiles: tests that read them are removed by default.
  - No crate under `crates/` depends on `dev/tools` or `experiments/`.
  - Gate before merge.
  - The relay rule.
  - Count commit subjects before every merge.

## History rewrite plan (45ecf975 REQ-04)

Unchanged since rehearsal 3, apart from the reword decision above.
- The runbook is [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md). The reword step reads [reword_subjects.py](45ecf975/reword_subjects.py).
- The superseded rehearsal directories were removed with the owner's approval.
- A fresh rehearsal from the then-current main must:
  - regenerate the delete list and the reword candidates by script;
  - merge in `reword-candidates-owner.tsv`;
  - add entries for every non-conforming subject after the candidates were drafted;
  - pass step 7 (cargo-ci in the rewritten checkout), which takes the lead's cargo-ci slot;
  - find no non-conforming subject left in `origin/main..main`.
- The bundle verify runs with `git -C backup.git`.
- The tracker split may change the rewrite's scope; the steward will ask about rehearsal steps.
- The owner gives the go. Nothing is pushed before it.

## Next

0. The steward handoff is `dev/active/2caf738d-1-0/handoff-3.md`. The d1c0e062 branch's hunk in `dev/active/2caf738d-1-0/handoff.md` has no target since `8cbf9eb17` retired that file: drop it when the branch next merges main.

1. **506f76d4: second review failure.** Escalated to the steward, who decides with the owner. The finding:
   - `dev/tools/tuning-campaign-support/src/receipt.rs:513` requires cold executions to disable calibration and supply `cold_calls` for every protocol version.
   - The deleted evaluator required both only from version 2. `FamilyAddendum::validate` keeps that distinction.
   - So the version-1 addendum `config/addendum-popcount-5ff212e5.json` fails after its first pair.
   - The fix passes the protocol version into the validation.
   Once it passes its gates, land it. It frees about 804 kB, unblocks 5f33fb28 and fe2221d5, and must land before da6200b8 and 0fe85425 are gated.
2. **d1c0e062:** gate it (cargo-ci, then code-review) and land it. It conflicts with nothing in flight.
3. **66fefd9e:** merge main into the branch, keeping `dispatch-7a816262-phase1.md` (removed by @/issue/d1c0e062) deleted, then gate and land.
4. **da6200b8:** after 506f76d4, as above.
5. **0fe85425:**
   - Merge main.
   - Drop its edits to `validate-tuning-extent-campaign.py` and the driver, which 506f76d4 deletes.
   - Drop its `crates/gf2-coding/tests/grand_phase1_smoke.rs` hunk; `@/issue/5c1c87b7` removed that test.
   - Remove gf2-coding's dev-dependency on tuning-campaign-support, rebasing onto main first.
   - Get gf2-5b's review of its four `scripts/cargo-ci.sh` steps, and gf2-d7's review of the `experiments/` files it touches.
   - Then gate.
6. **67194d70:** run its gates on main under a hold, then close it.
7. Then 5f33fb28, 95077f84, 411e7267, fe2221d5 if still wanted, and the rehearsal.

## Dispatch record (session 42)

The code-review gate's reviewer is Codex gpt-6.1-sol, so every implementation went to a Claude worker. Vendor alternation gave way to that separation. Codex built the independent inventory.

| Issue | Task | Vendor | Model / effort |
|---|---|---|---|
| 66fefd9e | inventory A, read-only | Claude | Sonnet 5.5 |
| 66fefd9e | inventory B, read-only, independent | Codex | gpt-6-luna, xhigh |
| da6200b8 | rework 1 | Claude | Opus 5.5 |
| 66fefd9e | rework 2 | Claude | Sonnet 5.5 |
| da6200b8 | rework 2, allow-list | Claude | Opus 5.5 |
| d1c0e062 | history retirement | Claude | Haiku 5.5 |
| 506f76d4 | rework 1 | Claude | Opus 5.5 |

## Traps

- A cold `cargo-ci` under load exceeds the 900 s gate timeout and records `error`. Warm the worktree with a direct `BUSY_HOST_OVERRIDE=1 ./scripts/cargo-ci.sh` (about 3 min warm), then evaluate the gate. Chain code-review only after cargo-ci passes.
- `jit gate evaluate` and `jit issue update` in a worktree record on that branch's `.jit`. After merging a gated branch, copy its `.jit/gate-runs/<issue-uuid>/` records into main's ignored `.jit/gate-runs/`.
- docs-mechanical needs main's `.jit/gate-runs/` in a worktree. Copy it into that worktree's ignored `.jit/gate-runs/` before running the check there.
- Main's working tree usually holds other leads' uncommitted tracker lines. Stage only your own paths and event lines, never check out or reset `.jit/events.jsonl`, and merge with plumbing (`git merge-tree`, `git commit-tree`, `git update-ref`). Plumbing skips the pre-commit hook, so run `scripts/check-tracked-budget.py --range` on the new commit first.
- `scripts/check-references.py` reads issue descriptions. Run it after every `jit issue update` and before committing tracker state. A bare file name in issue prose faults; write repository paths or plain words.
- The dispatch worktree script refuses while main has uncommitted tracker changes. Create the worktree with `git worktree add` from main.
- A gate result is evidence only through `jit gate status <id> <gate>`. Read findings there.
- A mechanical prose pass can change a runnable command or a criterion's meaning. Check code blocks and `## Success Criteria` lines after one.
- Sweeps by one agent miss wording forms. Slice by directory and reconcile two independent inventories by different vendors.
- Commit subjects from `wip(...)` commits and default merge messages reach main. Squash or reword on the branch, and count every subject with `${#s}`.
- Deleting machinery that validates committed artifacts can take live run-time checks with it (506f76d4, round 1). List what the deleted code checked on fresh results before deleting it.
- Dispatch and rework prompts are ephemeral coordination material and are never committed (owner directive 2026-08-24). Briefs live in `<gf2-share>/45ecf975-handoff-38/`.
- `git stash` is shared across worktrees and sessions. Never use it, and never put it in a brief.
- `rm -rf` outside the scratchpad is refused by the session's permission classifier unless the owner approves it in that session.
- `breakdown.json` stays excluded from audit by recorded owner decision (`240b7618`, `679cf170`). Do not argue its properties, edit it, or resync it. Do not amend an issue description without checking that issue's copy in it.
- A cold `cargo-ci` exceeds the 900 s gate timeout, and an `error` at that limit is not a verdict. Warm the tree with a direct run, then rerun the gate.
- Concurrent builds in other worktrees make fast-tier tests miss their 15 s kill. Check the gate record and rerun solo before reading a failure as a regression.
- GPU-device tests fail under host contention. Rerun the single test alone before reading it as a regression.
- Stage explicit paths and check `git diff --cached --stat`. `jit` materializes shared `.git/jit` state into the checkout that runs it, so a worker commit can carry other issues' `.jit/` changes.
- `jit doc add` is idempotent per path. Run `jit doc remove` on the old link first. An asset entry in a document link names the file too, so remove it with the link.
- Before deleting a file, remove its `jit doc` links and asset entries, then run `jit validate`. Do not close an issue whose text names a deleted path; rewrite the text first.
- Reference checks read tracked files only. A pass on an untracked file proves nothing.
- Do not rewrite a removed path to a bare base name. Point the citation at an existing file or follow the name with "(removed by @/issue/<id>)", and audit the whole class, not only the cited file.
- The history rewrite needs `map_pins.py`, because tracker documents pin commit IDs. Nothing is pushed before the owner's go; the pre-push hook refuses purged history.
- `codex exec` workers cannot write `.git`. They leave files in the tree, and the lead commits. Start a background `codex exec` with `< /dev/null`.
- Do not `git checkout --` or `git restore` a worker's uncommitted edits. Commit them as WIP first.
- A commit that changes public rustdoc needs `cargo doc`. Verify a commit's check results before committing; do not commit in the same command as a check whose result is only printed.
- Passing gates are stale once shared code changes, such as `experiments/common`. Re-gate.
- The AFF3CT adapter has its own `Cargo.lock`, which `cargo-ci` does not build. After a dependency change, build it with `--locked`.
- A reviewer re-run can surface new findings. Verify each one; read findings with `jit gate status <id> <gate> --findings`, not from reviewer rollouts.
- Do not edit `scripts/cargo-ci.sh` while an instance runs.
- A falsification must reinstate the real pre-fix code byte-for-byte. Exemption tests must check that the fixture reaches the exempted path.
- Host-wide process kills by command-line pattern can kill peer builds. Kill only PIDs from your own process tree.
- The shell is zsh, where `status` is read-only. Do not use it as a variable name in scripts.
- `git commit -F -` after an `&&` chain hangs. Write the message to a file.
- `/usr/bin/time` is not installed. Time commands with shell `date` arithmetic.
- Agent-tool sandboxed worktrees have vanished. Read the leak-check output and check peer sessions before reverting anything.

## References

- [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md), [reword_subjects.py](45ecf975/reword_subjects.py)

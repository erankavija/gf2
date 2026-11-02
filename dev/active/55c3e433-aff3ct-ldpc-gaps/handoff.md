# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 1

**Date:** 2026-11-01T21:20+02:00
**Session number:** 1
**Prior handoffs:** None.

## Current state

- Epic: `55c3e433`, state `backlog` (blocked by its sink; assigned to `agent:jit-execution-lead`).
- Wave in progress: wave 2 of 6. Pre-timing work is complete and reviewed by the lead. Gates have not run.
- Children summary: 1 done (`10590c11`), 2 in_progress (`1d787ed3`, `a72f2b3f`), 4 backlog, 0 rejected. Planning `c39b9e1b` and breakdown `228c4691` are done.
- Active claims: `1d787ed3` and `a72f2b3f`, assignee `agent:worker`. No worker is running.
- Open escalations: None. The owner approved the window below.
- Progress file: `progress.json` in this directory.

**Timer armed:** `gf2-bench-window-20261102`, 2026-11-02 04:00 Helsinki, runs `dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh`. At 21:15 the queue would run only two jobs, in order: `1d787ed3` (estimate 40 min), then `a72f2b3f` (estimate 25 min). State and logs are in `.agents/bench-window/`.

**This session's watcher ends with the session.** Nothing reports the window's end, and the next lead must check it.

## What just happened

- `10590c11`: done, all three gates passed at `1a2c4448b`. Codex `gpt-6.1-sol` delivered REQ-02 to REQ-05; Claude Opus delivered REQ-01. One code-review failure (replay panicked on layered AFF3CT) was fixed by the lead in `e84df68c0`.
- `a72f2b3f` (Codex `gpt-6.1-sol`): prepared at `27882bbb0`, on `worktree-agent-a72f2b3f`.
  - Both variants' row and posterior bits equal unchanged production.
  - Disassembly: `cached-signed` keeps both minima branches; `select-reduced` removes the second-minimum branch and keeps the first-minimum branch.
  - The issue's REQ-03 and REQ-05 remain.
- `1d787ed3` (Claude Sonnet): prepared at `3ea453e42`, on `worktree-agent-1d787ed3`.
  - QC bits equal canonical on the 8 timed frames (`pilot/qc-bits/result.txt`).
  - The issue's REQ-01, REQ-03 and REQ-04 remain.
- Neither wave 2 branch is merged to main. The queue lines are on main at `0b12bdceb`.
- Jit format 3 landed on main at `8c0da4457`, run by the owner's process.
- Peers told to hold from 03:50 until a finished report: `gf2-00` acknowledged; `just-in-time-c9` confirmed `just-in-time-aa`, `just-in-time-d0` and `gf2-00`.

## What to do next

- [ ] After the window, read `.agents/bench-window/window.log`. Check `job exit` rc for both jobs and the load recorded at each `job start`. Other sessions were asked to hold, but nothing enforces it. A loaded start goes to the owner before the receipts are used.
- [ ] Tell `just-in-time-c9` the window finished or overran, so held sessions resume. c9 asked for this, and `gf2-00` is waiting on it too.
- [ ] Commit in each worktree: the receipt directory under `55c3e433/`, plus `iterations.jsonl` for the pilot. `window.sh` commits nothing. The `iterations.jsonl` extractor is untested on a timed log.
- [ ] Merge main into both branches, after timing only. Run `jit validate` after each merge. On duplicates or failure, stop and tell `just-in-time-aa`.
- [ ] `1d787ed3`: write the decision record (issue REQ-03, REQ-04, REQ-05), applying `qc-go-rule` from `plan.md`. Merge, run the gates and complete it.
- [ ] `a72f2b3f`: write the nomination record (issue REQ-05), applying the rule in contract `check-variants`. Merge, run the gates and complete it.
- [ ] On a QC no-go: reject `4555dfb3` and `0cc6c195` with `jit issue reject --reason obsolete`, then go to wave 5. On go: wave 3, `qc-decoder-core`. It is the most demanding issue, so use Codex `gpt-6-astra` and watch for over-engineering.
- [ ] Wave 5 and any later confirmation need a new window, which needs fresh owner approval. This approval covers tonight's window only.
- [ ] Reclaim worktrees after their receipts are committed and merged. Update `progress.json` after wave 2.

**Owner's dispatch rules:** set the model by task difficulty, no Fable, alternate Codex and Claude. The last dispatches were Codex (`a72f2b3f`) and Claude (`1d787ed3`). Use Astra for the most demanding problems.

## Traps — do not repeat these

- **Do NOT land `inputs/` copies or raw logs from the pilots.** `@/inv/committed-evidence-budget` and `scripts/check-tracked-budget.py` now refuse them on main. Commit results, configuration and digests only. `git rm` the runner's `inputs/` and `*.log` output before merging; execution logs stay. `gf2-sim` aff3ct comparison helpers move under `@/issue/1aea3958`, last in `@/issue/45ecf975`; coordinate before that lands. A history rewrite is planned under `@/issue/45ecf975` and needs this epic's branches pushed or merged first.
- **Do NOT dispatch until the worktree is confirmed to exist.** The dispatch script refused a dirty main with exit 0 from my wrapper pipeline, and I launched two workers at missing directories. Check `git worktree list` first, in a separate step.
- **Do NOT merge main into a wave 2 worktree before its receipt exists.** The executables, build records and committed plan there are what the window runs. `pilot/window.sh` fails if regenerating the plan changes it.
- **Do NOT delete `target/` in either wave 2 worktree or reclaim it early.** Build records point there, including `target/variant-trees/` in `agent-a72f2b3f`.
- **`jit` in either worktree exits 10 until its branch merges main.** This is expected. A search found no `jit` call in the window scripts, campaign tools or lock wrappers.
- **Do NOT word-split a variable in zsh.** `git add $F` with a multi-line value failed. List paths explicitly.
- **`jit issue claim` fails on the blocked epic.** Use `jit issue assign`.
- **Do NOT accept a worker's "done" on signature changes without grepping callers.** `run_cell` gained a parameter and the `aff3ct-intra` caller was missed.
- **`arm-support/gf2-batch-single-frame-unchanged` has a dead `build_root`.** Its tree was removed. Rebuild with `record-gf2-arms-build.py OUT --arm NAME`, using `--supersedes` for a repeated name, or the older record wins the lookup.
- **The Claude worker for `1d787ed3` resent its final report repeatedly** while "waiting on background work" with none running. Stop it once the report is reviewed.
- **The AFF3CT ratio direction is gf2 time / AFF3CT time.** An evaluator `pass` means AFF3CT has a material advantage. QC against AFF3CT is indirect, and the iteration counts for AFF3CT are replay-derived.
- **Do NOT merge the 13 dormant branches `just-in-time-aa` listed.** Merging produces 48 to 363 duplicate event ids. Deleting them is the owner's call.

## Open questions needing invoker input

None blocking. The owner may still overrule the plan's comparator-family boundary (`plan.md`, Material risks).

## Reference artefacts

- Epic: `jit issue show 55c3e433`
- Planning docs: `plan.md`, `breakdown.json`, `investigation.md`, in this directory
- Arm evidence: `arm-support/` in this directory
- NR pilot, on `worktree-agent-1d787ed3`: `pilot/window.sh`, `pilot/nr-route-pilot.json`, `pilot/qc-bits/result.txt`
- Variants, on `worktree-agent-a72f2b3f`: `check-variants/README.md`, `check-variants/assembly.md`, `check-variants/posterior-comparison.txt`
- Expected receipts: `pilot/` and `check-variants/`

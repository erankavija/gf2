# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 9

**Date:** 2026-09-11T17:50Z
**Session number:** 9 (session 8 ended at a provider limit without a handoff)
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md), [session 4](handoff-4.md), [session 6 halt](handoff-5.md), [session 7](handoff-6.md)

## Current state

- Epic: `1a379447`, backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 2 of 7. It now also holds `bdc507a3`, `a387825e`, `428f2f6b`, `3e59cb9a` (no sibling dependencies) and `3be770d5` (both dependencies done).
- Children: 5 done, 7 in progress, 5 ready, 10 backlog, 0 rejected; `jit issue status` and [progress.json](progress.json) are authoritative.
- Active claims: no leases. Worker assignments on `3e59cb9a`, `bdc507a3`, `a387825e`, `3be770d5` (`agent:worker`); lead assignment on `04b85d10`, `26465e6c`, `6fb89a3c`.
- Open escalations: none. Five invoker decisions this session are recorded in progress.json.
- **Benchmark window armed:** transient user timer `gf2-bench-window-20260912` runs [run-window.sh](bench-window/run-window.sh) at 2026-09-12 04:00 Europe/Helsinki over [queue.tsv](bench-window/queue.tsv) (twelve jobs: 6fb89a3c ×3, 04b85d10, 6c6b09b1 ×3, 3e59cb9a, 3be770d5 ×4). Runtime log and per-job output: `.agents/bench-window/` (untracked).

### Per-issue state

| Issue | Branch (worktree) | State and next step |
|---|---|---|
| `c7113c5a` | merged | Done this session (`19097475`). |
| `26465e6c` | `worktree-agent-26465e6c-v3` at `b7c0283d`, checked out nowhere | Merged `85cd6415`, cargo-ci pass, linked `cd6e69d1`. Two independent reviews, code-review, research-review pass on `cd6e69d1`; **doc-review F1 fails**: `dev/bench_results/26465e6c/run-campaign.sh:20` promises a resume that never repeats a cell, false for a mid-cell interruption (P-11, `bdc507a3`). Rework round 1 = comment fix plus sweep; the worker drafted it read-only. |
| `04b85d10` | `worktree-agent-04b85d10-v3` at `69a3480e` | Rework round 1 done except the repeated profile (window job, pinned `c7de48e9`). |
| `6fb89a3c` | `worktree-agent-6fb89a3c-v3` at `f64cfa8e` | Rework round 1 done except three exploratory re-measurements (window jobs). |
| `6c6b09b1` | `worktree-agent-6c6b09b1-v3` at `b2ac9e7c` | v3 migration done up to three exploratory pilots (window jobs, pinned `7346b5e5`). Confirmations follow the pilots. |
| `3e59cb9a` | `worktree-agent-3e59cb9a` at `d23c0de1` in `.worktrees-local/agent-eda07788-v3` | REQ-01, REQ-02 met; exploratory re-measurement queued (pinned `fc527d62`). |
| `a387825e` | `worktree-agent-a387825e` at `7cf03070` in `.agents/worktrees/agent-26465e6c-v3` | Complete per worker; not merged. |
| `bdc507a3` | `worktree-agent-bdc507a3` in `.agents/worktrees/agent-c7113c5a-v3` | In flight at handoff; see the update log below. |
| `3be770d5` | `worktree-agent-3be770d5` in `.agents/worktrees/agent-3be770d5` | In flight at handoff; see the update log below. |

## What just happened

- Reconstructed session 8's end from branches and its transcripts under `~/.claude/projects/-home-vkaskivuo-Projects-gf2/4dd2dc73-68da-4918-ba03-2653c6d62e7b/subagents/`: lead and all four workers hit a provider limit at about 23:50Z; the 04:00 window ran with an empty queue.
- Closed `c7113c5a`: round 2 converged reviews on `1757b2a7`; code, doc and research gates pass; tables regenerate byte for byte.
- Dispatched continuations (Opus) for `26465e6c`, `04b85d10`, `6c6b09b1`, `6fb89a3c`, then `3e59cb9a`, `bdc507a3`, `a387825e`, `3be770d5`, reusing merged worktrees where possible.
- Invoker decisions: window tonight at 04:00; `a387825e` authorized as a shared-infrastructure change; `26465e6c`'s AND-popcount pilot-r2 and confirmation (run 23:18-23:24Z yesterday, after the no-timed-runs order the worker had not yet received) stand; `a387825e` REQ-01 reworded to lock level (protocol P-07 requires runners to inherit the held mutex descriptor); `3e59cb9a` REQ-03 reworded to "recorded in its family ledger" (attempt 3 cannot pass P-20, so the re-measurement is exploratory).
- Lead findings acted on: the `6fb89a3c` M4RI transpose kernel arm warmed through a fresh output instead of the timed call's buffers (same class as code-review F1). Fixed before queueing under a new executable identity; the jobs were held until then.
- Filed `835f34f0` (outside the epic, depends on `6c6b09b1`): `Gf2mField::gf256()` rustdoc names the AES polynomial 0x11B while the code builds 0x11D. Wired `c04dd4ac` → `3e59cb9a` (transitively reduced; the `1362381c` → `3e59cb9a` edge was dropped as redundant).
- Condensed worker reports (artifact-link tables, remaining steps) are in this session's scratchpad `reports/`. They are summarized in the next section, because the scratchpad does not survive a reboot.

## What to do next

- [ ] **After the window** (check `.agents/bench-window/window.log` and each job's `.out`; a job without a `.done` marker failed or never ran). Revive the owning worker per issue, or dispatch a continuation with the steps below. Merge main into a window branch only after its last timed run of the family.
  - `04b85d10`: check `repetitions.log` shows every session done, "series done" and one digest, and each `rep-*/verify.jsonl` reports 0 failed. Commit rep-02..rep-09, `asm/`, both summaries and the appended logs. Check every qualitative findings conclusion against the generated rows. Set REQ-03/REQ-04 to MET, sweep, merge main, cargo-ci, two independent reviews, gates. Links: the repeated profile's `profile-summary.md`, `attribution-summary.md`, `asm/popcnt-attribution.txt`, `repetitions.log`, `survey/run-profile.sh`; relabel or unlink the old `profile-v3/attribution-summary.md` link as history.
  - `6fb89a3c`: commit the three re-measurement receipts, extend `survey/summarize.py`, write results under SSOT, merge, reviews (Tier 1.5: code-review F1 and the lead's M4RI finding), gates.
  - `3e59cb9a`: commit receipt `dev/bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/` (force-add nested `Cargo.lock` snapshots) and ledger line 4, add it to `summarize-v3.py`, regenerate `tables-v3.md`, write outcomes in eda07788 findings §1. Link the eight artifacts it listed (re-measurement addendum and producing manifest, `survey/validation-output-v3-remeasure.txt`, `survey/freeze-remeasure.py`, the launcher, findings, source evidence, family ledger) to `3e59cb9a`.
  - `6c6b09b1`: commit the three pilot receipts and ledger lines, regenerate `tables.md`, derive each family's resolution (`survey-analysis resolution`), freeze confirmations (at most six confirmatory cells per family), queue them for the next window.
- [ ] `26465e6c` rework round 1: give it a worktree with `worktree-agent-26465e6c-v3` checked out (new worktree, or wait until `a387825e` merges and switch that directory back). Revive `w9-26465e6c` with the doc-review F1 text (it drafted the fix read-only). Then merge, run two independent reviews and all three AI gates on one commit, and close.
- [ ] Integrate `a387825e` after the window. Once it is on main, update the stale comment and log text in `run-window.sh` (lines 33-38 and 49). Warn every later window: worktrees keep their old wrapper copy until they merge main, and `ccx1-bench-flock.sh` is a producing input of nine survey manifests (04b85d10, 1d0da41f, 26465e6c, 6fb89a3c, a83583e0, c077a88b, c7113c5a, eda07788, f547c394).
- [ ] Collect `bdc507a3` and `3be770d5` from their branches (update log below). Queue any `3be770d5` window job for the next window.
- [ ] Remaining ready work, four workers at a time: `428f2f6b` (after `3e59cb9a` closes, since both edit eda07788 findings), `12fdeb5b`, `53c5a8c0`, `1c602857` (their dependencies are done; move them into the current wave as `3be770d5` was).
- [ ] Offer the invoker the open `26465e6c` question below.

## Traps — do not repeat these

- **Do NOT reuse a worktree directory for a new issue before its issue closes.** `26465e6c` failed doc-review after its directory was switched to `a387825e`; its rework had nowhere to run and nearly committed onto `a387825e`'s branch. Reuse only a done issue's worktree.
- **Do NOT message the `w8-*` teammates.** They still appear as idle in `ListAgents` and a message revives them against worktrees that other workers now own.
- **Do NOT expect lead messages to reach a worker inside a long tool call.** Messages arrive between tool calls; session-8 workers in 5-8 minute sleep loops got the timed-run orders 20-50 minutes late and ran campaigns after the order. Briefs now require polling in calls of at most about two minutes.
- **Do NOT stop a worker's background tasks while it runs campaigns.** Stopping them killed every process the worker's tool calls spawned, setsid or not (session 8: three launchers died mid-cell, one receipt rejected on P-11).
- **Do NOT promise a cell-safe resume after any interruption.** Under protocol v3 a mid-cell interruption restarts the cell and acceptance rejects the stage (P-11) until `bdc507a3` lands; the window runner therefore never interrupts a job. Other surveys' launchers may carry the same sentence `26465e6c`'s did.
- **Do NOT accept a warm pass that uses different buffers from the timed call.** `6fb89a3c`'s M4RI kernel arm warmed through a fresh output; the protocol's warm pass is the timed body over the timed call's working set. `survey/verify-warm-pass.py` on the `6fb89a3c` branch checks C arms under gdb without timing.
- **Do NOT write a queue line with an absolute worktree path.** The runner joins it to the repository root; normalize to the repo-relative path (`04b85d10`'s line needed it).
- **Do NOT treat `Tree: dirty` on a gate run as a code change when only `.jit/events.jsonl` and the issue JSON differ.** `jit claim acquire` and the gate evaluation write them; `git status` before and after tells the difference.
- **Do NOT restore a missing file into a committed receipt snapshot without the invoker's explicit decision.** The harness refuses it as audit tampering (`26465e6c`'s 2026-09-07 v1 pilot `Cargo.lock`).
- Unresolved traps from [session 7](handoff-6.md#traps--do-not-repeat-these), [session 6](handoff-5.md), [session 4](handoff-4.md#traps--do-not-repeat-these), [session 3](handoff-3.md#traps--do-not-repeat-these), [session 2](handoff-2.md#traps--do-not-repeat-these) and [session 1](handoff.md#traps--do-not-repeat-these) remain in force.

## Open questions needing invoker input

- Question: restore the missing `Cargo.lock` into `dev/bench_results/26465e6c/2026-09-07-26465e6c-popcount-pilot/inputs/producing/`?
  - Context: that superseded v1 pilot pins a `Cargo.lock` (sha256 `541e7dcc…`) the v1 branch never committed. The current acceptance tool rejects it on P-05; three sibling v1 snapshots hold a byte-identical file. The harness refused the restore as audit tampering.
  - Options: A) leave it; `reevaluation.txt` and findings history state the gap; B) restore the identical-digest copy, rerun `survey/reevaluate.sh`, commit.
  - Recommendation: A. No result rests on that pilot, and the gap is recorded.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [progress](progress.json); protocol `dev/active/f547c394/protocol.md` (v3).
- Window: [run-window.sh](bench-window/run-window.sh), [queue.tsv](bench-window/queue.tsv); `systemctl --user list-timers | grep gf2`.
- Session-9 briefs: common rules and epic worker rules in the session scratchpad (`s9-common.md`, `epic-worker-rules-s9.md`, `prompts/s9-<id>.md`); the independent-review helper is `indep-review.sh` there (session 7 describes it). Recreate from these descriptions if the scratchpad is gone.
- Gate records: `.jit/gate-runs/`; session-8 and session-9 worker transcripts under `~/.claude/projects/-home-vkaskivuo-Projects-gf2/4dd2dc73-68da-4918-ba03-2653c6d62e7b/subagents/` (`agent-aw8-*`, `w9-*`).
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.

## Update log

- Initial write at 17:50Z while `bdc507a3` and `3be770d5` commit their state.
- 17:55Z `bdc507a3` reported complete at `8dbc6406` on `worktree-agent-bdc507a3` (not merged): protocol v4 with a `cell-abandoned` journal record and v4-only P-11 rules, `amendment-v4.md`, schema v4. `dev/active/bdc507a3/verdict-preservation.json` shows identical verdicts and summaries for all 59 committed receipts between `3aa2920b` and the head. **Before merging, decide the sequencing:** once merged, main's runner refuses new campaigns from v3 addenda. So every v3 family (6c6b09b1's confirmations, the window re-measurements) must finish on its own worktree, or move to v4 addenda once it merges main. v4 also gives each candidate one more confirmatory attempt, since the cap counts per protocol version, while earlier attempts still lower alpha. Reviewers will read `amendment-v4.md` for that. Links after merge: `dev/active/f547c394/{protocol.md,amendment-v4.md,addendum.schema.json,design.md}`, `dev/active/bdc507a3/verdict-preservation.json` (report), `dev/active/bdc507a3/compare-acceptance-verdicts.sh` (benchmark).
- 17:57Z Filed `a203a23c` (in-epic bug, depends on `bdc507a3`, feeds `1362381c`): 14 committed receipts are rejected on a fresh checkout although their committed summaries say accepted. Most lack gitignored snapshot `Cargo.lock` files. They include 1d0da41f's governing v3 receipts and c077a88b's r3/r4 pilots. Restoring bytes needs the invoker's approval (same harness block as the 26465e6c v1 pilot).
- 18:00Z Queued three `3be770d5` exploratory pilots (single-worker, multicore, fastest-compatible), pinned `cbacdf99` on `worktree-agent-3be770d5`. The queue now totals about 160 estimated minutes from 04:00. The worker's repeated profile series (about 40 min) was not ready and goes to the next window.
- 18:05Z The `3be770d5` repeated steady-state profile (nine sessions, about 45 min) was committed at `3507fd4d` before the cutoff and is queued after the pilots; the three pilot lines are unchanged by that commit. The queue totals about 205 estimated minutes from 04:00.

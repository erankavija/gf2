# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 14

**Date:** 2026-09-28T13:40Z
**Session number:** 14
**Prior handoffs:** `handoff.md` through `handoff-13.md`

## Current state

- Epic: `6dc81018` — state: in_progress
- Wave in progress: wave 27 of 28 (`a83583e0`); wave 28 is `dbd8787d`
- Children summary: 11 done, 1 backlog (`dbd8787d`, blocked on `a83583e0`)
- Active claims: `a83583e0` assigned to agent:jit-execution-lead, in progress
- Open escalations: none
- Progress file: `progress.json` in this directory (reflects the above)
- **Queued measurement:** the fresh 717-cell extent campaign is the last row of
  `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv`, after the
  M4RI pilot. Worktree `.agents/worktrees/agent-a83583e0-run`, detached at
  `eebbc778c`, clean, release executables pre-built. Transient timer
  `gf2-bench-window-20260929.timer` fires 2026-09-29 02:00 Europe/Helsinki
  (2026-09-28 23:00 UTC) and runs `run-window.sh` from main.

## What just happened

- Invoker approved option C again this session and allowed the protocol edit.
  Protocol §7 now states that host admission is the exact static
  `HostAdmissionPolicy` plus the full-host outer lock; load and memory are
  descriptive only.
- Opus worker made `HostAdmissionPolicy` truthful in driver and validator
  (`admission: full-host-outer-lock`, `host_observation: descriptive`, exact
  `recorded_observations` = serialized `HostObservation` fields). The driver now
  journals one held-lock `host-observation` diagnostic, which the validator
  checks. Before this, the driver never called `HostObservation::observe()`.
- Lead added `host.rs`, `process.rs` to all three producing-manifest lists and
  the other linked support modules to `build_inputs`; added `host.rs` and
  `process.rs` to the §10 boundary. Committed at `15d47f3bc`.
- Support tests 193/193, validator self-test, focused core/algebra tuning tests
  and full `cargo-ci.sh` pass. Gates `cargo-ci`, `code-review`, `doc-review`
  on `a83583e0` passed (premeasurement state).
- Interactive launch built, staged and self-checked, then `run-session` exited 3
  before `LockHeld`: `ccx1-bench-flock.sh` refuses the mutex outside the
  benchmark window. Terminal `failed`, zero accepted units. Small evidence is
  preserved in `dev/active/a83583e0/failed-attempts/gf2-a83583e0-20260928T130755Z-3719910/`;
  the full stage stays at that `/tmp` path.
- Queued a fresh campaign for tonight's window (commit `eebbc778c`).
- Freed disk: deleted main's rebuildable `target/debug` and `target/ci-test`
  (24 GB → 93 GB free). Worktrees untouched; ~157 GB remains in
  `.agents/worktrees`, mostly done/merged issues.

## What to do next

- [ ] After the window, read `.agents/bench-window/window.log`, the job's
  `<key>.out`, and the campaign's own `execution.log` (path printed as
  `GF2_CAMPAIGN_EXECUTION_LOG=` in `<key>.out`). A zero exit alone is not proof.
- [ ] If the session ended `budget-exhausted` or interrupted, re-queue the same
  campaign ID (`dev/scripts/tuning-extent-campaign.sh <campaign-id>`) in the
  same worktree for the next window; never start a new identity for a
  resumable campaign.
- [ ] On `complete`: run `dev/scripts/validate-tuning-extent-campaign.py` on the
  stage, carry the published envelopes/receipt/evidence from the worktree onto
  main, re-run `a83583e0` gates on the final tree, apply the six-tier lead
  review against REQ-01..05, and transition `a83583e0` to done.
- [ ] Wave 28: implement `dbd8787d` (three seam threshold fields). Its timed
  sweep must also go through the window queue.
- [ ] Then epic gates, completion report, transition, archive.
- [ ] Optional disk relief: done+merged worktrees' `target/` dirs are large and
  hard-linked; ask before deleting anything pinned.

## Traps — do not repeat these

- **Do not launch any timed run interactively.** Since invoker policy
  2026-09-13 (`dev/scripts/ccx1-bench-flock.sh:78-84`), the host mutex exists
  only inside the benchmark window. Queue in the 1a379447 `queue.tsv` with a
  clean pinned worktree. The earlier handoffs' "run through the single outer
  lock" steps predate that policy.
- **Do not touch `.agents/worktrees/agent-50f0bd42` or reorder the queue.** The
  M4RI pilot belongs to epic 1a379447 and runs first.
- **Do not move or dirty `agent-a83583e0-run` before the window.** The launcher
  requires a clean tree and pins source identity at build time.
- **Do not expect live host-admission evidence.** Option C records the host
  observation descriptively; the validator checks only its field set.
- Prior traps remain in force (driver/validator agreement, atomic publication,
  separate code/JIT commits, no Astra/Fable-class subagents).

## Open questions needing invoker input

None.

## Reference artefacts

- Protocol: `dev/active/a83583e0/premeasurement-protocol.md` (§7 amended)
- Option C commit: `15d47f3bc`; queue commit: `eebbc778c`
- Failed attempt: `dev/active/a83583e0/failed-attempts/`
- Window runner: `dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh`
- Timer units: `/run/user/1000/systemd/transient/gf2-bench-window-20260929.{timer,service}`

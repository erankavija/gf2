# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 16

**Date:** 2026-10-01
**Session number:** 16
**Prior handoffs:** `handoff.md` through `handoff-15.md`

## Current state

- Epic: `6dc81018` — state: in_progress
- Wave in progress: wave 28 of 28 (`dbd8787d`)
- Children summary: 11 done, 1 in progress (`dbd8787d`, assigned agent:worker)
- Open escalations: none
- Progress file: `progress.json` in this directory
- **Queued measurement:** transient timer `gf2-bench-window-20261002.timer`
  fires 2026-10-01T23:00Z (02:00 Europe/Helsinki) and runs
  `dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh`. The
  only unfinished queue row is `dbd8787d`
  (`dev/scripts/tuning-extent-campaign.sh dbd8787d`), run from
  `.agents/worktrees/agent-dbd8787d-run`, detached at `87b5b733c`, clean.
  The launcher builds the producers and mints a `gf2-dbd8787d-…` run ID.

## What just happened

- a83583e0 campaign `gf2-a83583e0-20260930t230000z-2728298` completed and
  published. Cutover `e100821df` (v4 owners and complete envelope as current
  readers, baked GEMM 32/64 and dot 256 retained as conservative, v3
  rejected). doc-review required an evidence index for the pinned receipt
  (`…-evidence.md`) and present-tense doc fixes. All gates passed at
  `ad30c52ad`; a83583e0 done. Stage copied to `.agents/campaign-stages/`;
  archive on main at `.agents/campaign-evidence/<run-id>/`.
- dbd8787d REQ-02 amended with invoker approval (`120989525`).
- dbd8787d premeasurement implementation `87b5b733c`: three seam fields in
  the retained-threshold phase (core 756 cells, est. 7,800–7,950 s active),
  PLE route observer, per-campaign `campaign-declaration.json` read by
  launcher/driver/validator, imported a835 algebra owner pinned by digest,
  `publish-campaign` emits the evidence index. Protocol
  `dev/active/dbd8787d/premeasurement-protocol.md`.
- Gates at `6a53adc35`: cargo-ci passed; code-review and doc-review fail
  only on findings that the campaign is not yet executed/published.

## What to do next

- [ ] After the window read `.agents/bench-window/window.log`, the dbd job's
  `.out` (key from the log), and the execution log it names. Require terminal
  `complete` and `publication` status `published`. A zero exit is not proof.
- [ ] Copy the stage from `/tmp` to `.agents/campaign-stages/` immediately.
- [ ] Copy the committed destinations from the run worktree onto main and the
  archive to main's `.agents/campaign-evidence/`; re-run
  `validate-tuning-extent-campaign.py --stage … --publication <main>`.
- [ ] Single cutover commit: dbd core owner and complete envelope as current
  readers (algebra owner stays the a835 file), baked citations refreshed,
  `tuning_repository_envelopes.rs` mixed IDs, protocol status line set to
  executed. Then all dbd8787d gates, lead review, done.
- [ ] Epic gates, completion report, transition, archive.

## Traps — do not repeat these

- **Keep the host idle 23:00Z to ~02:30Z.** Builds outside the window are
  not locked out (handoff-15 trap).
- **Do not commit or dirty `agent-dbd8787d-run` before the window.**
- **The receipt must cite only repo-relative or archive paths.** a835's
  receipt cited `/home/...` and bare stage names and failed doc-review; the
  dbd driver now renders an evidence index, but check it.
- **Present-tense docs:** doc-review flags any retained "optimal"/"pending"
  claims superseded by a new measurement (KERNEL_OPTIMIZATION.md Phase 4
  lines, protocol status lines).
- **Do not `git commit -a` after a gate run;** it sweeps `.jit` into a code
  commit (happened in `ad30c52ad`).
- Prior traps remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Commits: `e100821df`, `ad30c52ad`, `120989525`, `87b5b733c`, `6a53adc35`
- a835 evidence index: `dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298-evidence.md`
- Timer: `gf2-bench-window-20261002.{timer,service}` (transient, user)

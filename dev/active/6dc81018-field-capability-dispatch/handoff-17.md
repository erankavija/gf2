# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 17

**Date:** 2026-10-02
**Session number:** 17
**Prior handoffs:** `handoff.md` through `handoff-16.md`

## Current state

- Epic `6dc81018` in_progress; wave 28 of 28 (`dbd8787d`, in_progress, agent:worker).
- Campaign `gf2-dbd8787d-20261001t230000z-2601601` measured all 756 cells
  (4,536 accepted results) and composed `complete.json`; its finalize-time
  validator rejected the stage on a redundant campaign-index key list lacking
  `imported_owners`, so the journal ends `failed`.
- Stage: `/tmp/gf2-dbd8787d-20261001t230000z-2601601`; backup
  `.agents/campaign-stages/gf2-dbd8787d-20261001t230000z-2601601`.
- Fix `cb5708396` on branch `wt/dbd8787d-publish`
  (worktree `.agents/worktrees/agent-dbd8787d-publish`), not merged. Invoker
  approved the protocol lightening (2026-10-02): identity sources read at the
  producing revision, validator identity informational, a failed terminal
  caused only by the validator verdict counts as complete. Recorded in
  `dev/active/dbd8787d/premeasurement-protocol.md` §10.
- Fixed validator reports the stage `complete-valid`. Release driver built at
  `target/release/tuning-extent-campaign-driver` in that worktree.

## What to do next

- [ ] Publication (blocked on the invoker; the auto-mode classifier denies it
  to agents). From the publish worktree:
  `RUSTUP_TOOLCHAIN=1.95.0 RAYON_NUM_THREADS=4 target/release/tuning-extent-campaign-driver publish-campaign /tmp/gf2-dbd8787d-20261001t230000z-2601601`
- [ ] `cp -a` the worktree's `.agents/campaign-evidence/<run-id>` to main's;
  run `validate-tuning-extent-campaign.py --stage … --publication <worktree>`.
- [ ] Cutover per handoff-16; compare the three seam selections with retained
  values; `./scripts/cargo-ci.sh`; merge to main; dbd8787d gates and done.
- [ ] Epic gates (holistic-review last failed 2026-08-22), completion report,
  transition, archive.

## Traps — do not repeat these

- **Agents cannot run the validator/driver against the stage.** The auto-mode
  classifier blocks the publication trajectory for subagents even with relayed
  invoker approval, and blocked the lead's `publish-campaign` too. Ask the
  invoker to run it or add an allow rule; do not re-dispatch a worker for it.
- **main may carry another session's uncommitted `.jit` edits**;
  `dispatch-worker-worktree.sh` refuses then. Create the worktree with
  `git worktree add -b <branch> <path> main`, and commit only explicit paths.
- Prior traps remain in force.

## Open questions needing invoker input

- Run the publish command above, or allow it.

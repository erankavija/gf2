# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 15

**Date:** 2026-09-30T21:10Z
**Session number:** 15
**Prior handoffs:** `handoff.md` through `handoff-14.md`

## Current state

- Epic: `6dc81018` — state: in_progress
- Wave in progress: wave 27 of 28 (`a83583e0`); wave 28 is `dbd8787d`
- Children summary: 11 done, 1 backlog (`dbd8787d`, blocked on `a83583e0`)
- Active claims: `a83583e0` assigned to agent:jit-execution-lead, in progress
- Open escalations: none
- Progress file: `progress.json` in this directory (reflects the above)
- **Queued measurement:** transient timer `gf2-bench-window-20261001.timer`
  fires 2026-09-30T23:00Z (02:00 Europe/Helsinki) and runs
  `dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh`. The
  only unfinished queue row is the `a83583e0` extent campaign (key
  `9ec8534d97f72766`, no `.done` marker), run from
  `.agents/worktrees/agent-a83583e0-run`, detached at `21790510c`, clean,
  release executables pre-built. The launcher mints a fresh lowercase ID.

## What just happened

- The 2026-09-29 window campaign `gf2-a83583e0-20260928T230349Z-632396`
  measured all four phases (4302 accepted cells, ~2.1 h active), then core
  emit-owner failed: `ProfileId::parse` rejects the uppercase `T`/`Z` in the
  run ID, which is also the profile ID. Terminal `failed`. Its stage is copied
  to `.agents/campaign-stages/` (git-excluded via `.git/info/exclude`); it
  cannot be reused because emission pins the producing executable digests and
  the ID is baked into every unit key.
- `1b30a5f29`: launcher/driver/validator mint and require lowercase
  `yyyymmddthhmmssz`; both owner manifests reject a non-`ProfileId` campaign
  ID before any timed cell; core emit-owner end-to-end test.
- code-review then failed: no publication to repository destinations.
  `47da059e7` + `9cf955df8`: driver `publish-campaign` mode, run by the
  launcher after finalization and on relaunch; durable, idempotent, strict
  reopen through the staged composer; validator `--publication`.
- Invoker decisions: bulk evidence stays out of git ("large files shall not
  pollute the repo"). Committed set is envelopes, receipt, checksum manifest
  and bounded session records, hard 1 MiB limit per file (driver and
  validator). Everything else goes to git-ignored
  `.agents/campaign-evidence/<run-id>/`, pinned via an archived `SHA256SUMS`
  whose digest the committed manifest records. Receipt summarizes decision
  samples (`sample_count`); raw index and full decisions are archived.
  Full-scale worst case receipt 91.5 KB.
- Replaying the failed stage through the validator found seven
  driver/validator bookkeeping mismatches (start_sequence, typed-field
  digests, checkpoint case digest, owner-validation record order, route and
  reason spellings, float canonicalization). All fixed in `9cf955df8` with
  cross-language encoding and receipt round-trip tests. Emission,
  composition and reopen were dry-run on real (re-keyed) data.
- code-review then flagged the host-local archive against REQ-03. Invoker
  chose to lighten the protocol: REQ-03 amended (`7982d2c6f`), protocol §9
  states the archive is host-local. Migration-history comments rewritten
  (`21790510c`).
- Gates at `21790510c`: cargo-ci and doc-review passed earlier on the tree;
  code-review fails with one finding only — the executed campaign's committed
  artifacts and reader cutover do not exist yet. That clears only after the
  run.

## What to do next

- [ ] After the window read `.agents/bench-window/window.log`, the job's
  `9ec8534d97f72766.out`, and the campaign's `execution.log` (path printed as
  `GF2_CAMPAIGN_EXECUTION_LOG=`). Require terminal `complete` and a successful
  `publish-campaign`. A zero exit alone is not proof.
- [ ] If `budget-exhausted` or interrupted: re-queue the same campaign ID
  (`dev/scripts/tuning-extent-campaign.sh <campaign-id>`) in the same
  worktree; a relaunch resumes publication before preparing a session.
- [ ] On `complete`: run `validate-tuning-extent-campaign.py --publication` on
  the run worktree; copy the committed destinations from the worktree onto
  main and the archive to main's `.agents/campaign-evidence/`; make the single
  §9 cutover commit (v4 owners + complete envelope as current readers,
  `baked.rs` constants and citations, remove `PREPUBLICATION_HARNESS_SCHEMA`,
  v3 rejection test). Re-run all `a83583e0` gates, six-tier lead review
  against REQ-01..05, transition to done.
- [ ] Wave 28: `dbd8787d`. Its REQ-02 wording ("receipt records every ...
  timing sample") will collide with the no-bulk-in-git rule the same way;
  apply the same REQ-03-style amendment with invoker approval before gates.
  Its timed sweep also goes through the window queue.
- [ ] Then epic gates, completion report, transition, archive.

## Traps — do not repeat these

- **Other work during the window contaminates the timing.** Outside the
  window `scripts/cargo-budget.sh` and `ccx1-bench-flock.sh` run unlocked
  (`cargo-budget.sh:104-111`), so a parallel session's builds or tests do not
  wait for the campaign's full-host lock, and host admission is descriptive
  only (option C), so contention is not detected. Keep the host idle
  2026-09-30T23:00Z to about 02:30Z.
- **Do not reuse a campaign ID across a code change.** Emission refuses any
  executable whose digest differs from the producing one; the ID is part of
  every unit key.
- **A clean premeasurement gate pass is not an end-to-end check.** Last
  night's failure and the seven validator mismatches were invisible to unit
  tests; replay changes against the preserved stage
  (`.agents/campaign-stages/gf2-a83583e0-20260928T230349Z-632396`) before the
  next window.
- **Do not move or dirty `agent-a83583e0-run` before the window.** The
  launcher requires a clean tree.
- **`/tmp` is tmpfs.** Copy any stage worth keeping to disk.
- **Arming the timer:** `systemd-run --user` needs
  `XDG_RUNTIME_DIR=/run/user/1000` and
  `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`; the permission
  classifier may flag it as persistence, so confirm with the invoker first.
- Prior traps remain in force (no interactive timed runs, queue order,
  separate code/JIT commits, no Fable/Astra-class subagents).

## Open questions needing invoker input

None.

## Reference artefacts

- Protocol: `dev/active/a83583e0/premeasurement-protocol.md` (§9 publication,
  destinations table, host-local archive)
- Commits: `1b30a5f29`, `47da059e7`, `9cf955df8`, `7982d2c6f`, `21790510c`
- Failed stage copy: `.agents/campaign-stages/gf2-a83583e0-20260928T230349Z-632396/`
- Timer: `gf2-bench-window-20261001.{timer,service}` (transient, user)

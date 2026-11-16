# Handoff: gf2 1.0 release (2caf738d), steward session 5

**Date:** 2026-11-16
**Lead handoffs:** `dev/active/6ba9be17-tracker-repository/handoff-4.md` (cutover
line, the cutover record and the working rules), `handoff-3.md` there
(supporting line), `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-7.md`.
The streams `d77176e5`, `ae03bcd0` and `45ecf975` keep their handoffs of
2026-11-15. They hold the issue-level state.

## Current state

- The tracker cutover `@/issue/6889e6e6` ran on 2026-11-16, 01:43 to 01:52 local. The tracker is its own Git repository mounted at `.jit/` of the primary checkout; `main` tracks no path under `.jit/`. Its home is the mount's origin (`git -C .jit remote get-url origin`).
- Every session is wound down by the owner's order. No lead, worker, gate or lease runs. Nothing is pushed in the code repository; the pre-push hook refuses `main` on budget findings in unpushed history.
- Landed and closed this session: `574ed420`, `697b9178`, `9c0568ee`, `d8aee1f4`, `6d5ba9fb`, `28aa2a9d`, `3fb4accb`, `daa33be8`, `6310679b`, `7854cfad`.
- Open with recorded code-review findings, gates cargo-ci and landed passed: `e610f5c4` (tool retired after its run, in history at `6c886acb5`), `6b9e6b14`, `6889e6e6`.
- Charter decisions D-10 (cutover alone, before the rewrite), D-11 (`jit:none`) and D-12 (stale branches abandoned, the rest cleaned by a removal-only commit) are logged.
- Branches: the owner had every branch merged into `main` deleted, with the worktrees of 49 of them; 46 done-issue branches sit under `refs/archive/`; 46 local branches remain. Eight detached worktrees keep a stale `.jit` directory as named exceptions; the cutover handoff lists them.

## Owner decisions of this session

- Cutover alone, rehearsed alone, forward-only tool, rollback by git; the gates of its three issues ran after the window.
- Rewrite rewords: `42e42101a94d`, `797e1d5205af`, `79f9117d417b`, `f57fbec1e3d4`, `fceb1c842c57` take `jit:b4b4b9ee`; `a33af036b983` and the eleven free-scope subjects take `jit:none`. The 26 branches without an issue are deleted at the rewrite. All in the Notes of `@/issue/45ecf975`.
- Pushes stay held until the rewrite. A local bundle of all refs replaces the online backup.
- `18276ff1`: the budget-gate self-test waits on events only (option (a)).
- jit: run records may be declared machine-local (jit issue f25d6fda, approved by the owner on the jit side); gf2's plan decision D-07 stands.

## Steward rulings that bind the leads

- Leads set each subagent's model by task difficulty, never Fable, alternate Claude and Codex per dispatch, and review with the other vendor. Every brief says cargo runs through the host cargo shim.
- Linked worktrees run jit through `scripts/tracker-env`. After a snapshot, `git -C .jit push origin main` keeps the tracker's home current.
- A jit binary is changed on this host only together with the store changes it needs, and never during gf2 work in flight.
- A branch cleaned at the cutover merges `main` when it resumes; every gate reruns on the head that lands.
- `d1eba151` stays parked: the scope of unusual `env` interpreter lines is the owner's to rule.

## What to do next

1. Read the cutover handoff and run one landing in the new flow as the first act: `4f395137` has both gates passed on record.
2. Take the next jit binary in one step when the jit project reports f25d6fda merged: set its config key for machine-local run records, then check `jit validate` in a fresh clone through `scripts/tracker-bootstrap`. Track it under `@/issue/84022ee3`.
3. Finish `18276ff1`: the REQ-06 run of six concurrent cargo-ci runs with the test-thread sampler, the two recorded advisories, final gates, steward approval of the gate set. It ends the 15 s test kills under load.
4. Settle the three open cutover issues with the owner: fix the recorded findings or close them with the tool retired.
5. Then `2a4c2684`, the parked `b4b4b9ee` branches two at a time, and the paused streams on the owner's word.

## Traps

- The installed jit reports version 1.0.0 across behavior changes. The binary installed on 2026-11-16 at 01:10 local rejects presets that embed gate definitions and reports a gate record without its run file as a validation error; a fresh clone of the tracker therefore fails validation until f25d6fda is installed and configured.
- The jit project's session installs jit into the path gf2 uses. Its install hold was to end on the message "gf2 cutover done", which could not be delivered: its session had closed.
- The tree limit of `scripts/tracked-budget.toml` has about 79 kB of room and only falls (`@/issue/8660c096`).
- `git worktree remove` deletes ignored files. Check `git status --porcelain --ignored` first; the 49 merged worktrees were removed after a check of tracked and untracked files only.
- `jit validate` of the earlier binary passed on a store whose gate records named absent run files while `jit issue show` failed; load every issue after a port of gate records.
- A cross-session relay of an owner approval does not authorize a lead to write refs or the store; the owner types it in that lead's session.
- Earlier traps in the lead handoffs still bind: commit subjects, zsh word splitting, kills by working directory, one landing on `main` at a time.

## Open questions for the owner

- When the history rewrite runs, and the force-push with it.
- `@/issue/8660c096`: headroom for the tree limit.
- Whether `scripts/tracker-snapshot` pushes to the tracker's home by itself.
- The eight worktree exceptions: keep, or remove after saving their ignored content.
- `e610f5c4`, `6b9e6b14`, `6889e6e6`: fix the findings or close with the tool retired.
- `d1eba151`: scope of `env` interpreter-line spellings.
- `41517289`, filing the amdgpu report upstream, stays a deferred owner action.

## References

- Charter: `dev/research/2caf738d-charter.md`.
- Carve-out plan: `dev/active/6ba9be17-tracker-repository/plan.md`.
- Rewrite runbook: `dev/active/b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md`.

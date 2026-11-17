# Handoff: gf2 1.0 release (2caf738d), steward session 6

**Date:** 2026-11-17
**Prior handoff:** `dev/active/2caf738d-1-0/handoff-5.md`. Its traps still bind
unless resolved below.
**Lead handoffs of this session:**
`dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-8.md`,
`dev/active/d77176e5-competitive-benchmarking/handoff-9.md`,
`dev/active/6ba9be17-tracker-repository/handoff-5.md`,
`dev/active/b8206228-permanent-statistics/handoff-41.md` (the rewrite line).

## Current state

- The history rewrite ran on 2026-11-17, 01:36 to 01:53 local, and `main` is pushed to `origin`. `main` has 7950 commits and a 50 MiB pack; every commit ID changed. The tracker is pushed to its home.
- Every lead is wound down by the owner's order through the steward. No lease is held. `84022ee3` stays in progress, parked, assigned to the closed rewrite lead.
- 18 branches of open issues survive as rewritten refs. Three worktrees exist: the primary checkout and the bench arms `agent-de311215` and `agent-fd9d5416`, plus the run worktree `jit-3461a52a-run`.
- Rollback material is on the host under the `pre-rewrite` directory of the gf2 data directory: the old `.git`, mirrors and bundles of both repositories, the commit map named by its digest, and archives of the removed worktrees' ignored content. It stays until the owner says otherwise.
- A benchmark window is armed for 2026-11-17 04:30 local (user timer `gf2-bench-window-20261117`): the two `@/issue/3461a52a` curves, then a quiet-host cargo-ci baseline whose output lands beside the build-loop report in the gf2 data directory.

## Landed and closed this session

- Tech debt: `4f395137`, `22bb2dcd`.
- Tracker workflow: `a5773e5b`, `c568cb92`, `63813d52`, `3f10673e`, `673cd2bc`, `a6d0c14a`, `0cb65df5`, `c4d14911`, `8660c096`, `8d6a135f`.
- Rewrite: `2a4c2684`, `23c47bea`, the mechanical old-ID pass under `45ecf975`, and `3608a601` (the 163 remaining old-ID references, fixed by reviewed low-tier workers).
- Rejected by the owner with findings on record: `e610f5c4`, `6b9e6b14`, `6889e6e6`.

## Owner decisions of this session

- Charter D-13 (tree limit with 256 kB headroom) and D-14 (the rewrite carries open-issue branches; no committed commit map).
- Tracker push stays opt-in (`scripts/tracker-snapshot --push`); the plan's D-12 row is amended under `@/issue/a9ddca84`.
- The eight detached worktree exceptions are resolved: two bench arms kept without a store, six removed.
- `@/issue/d1eba151`: the checker reads two interpreter-line forms, a direct interpreter path and `env` with the interpreter name and no options.
- The cargo-ci gate timeout is 1800 s, in the live registry and the package source.
- `@/issue/18276ff1` lands only after a passing REQ-06; attempt 3 failed and the issue is parked with its record.
- The content-standards sweep is left alone.
- The build-loop investigation is free-form: its report and data are in the gf2 data directory, and it files no issues.

## Steward rulings that bind the leads

- Leads set each subagent's model by task difficulty, never Fable, alternate Claude and Codex per dispatch, and review with the other vendor. Every brief says cargo runs through the host cargo shim.
- Landings follow `dev/index.md` "Landing flow". The merged-tree step judges cargo-ci only. A lead requests done before the `landed` gate runs: jit completes an issue on its last gate pass only from state `gated`.
- A store-sourced projection changes in the store when the branch carrying the tracked copy enters review, and the merge follows promptly (`@/issue/121f55fb`). No `jit validate --fix` in the primary checkout during that window.
- `scripts/check-references.py` is run in the primary checkout after every issue edit (`@/issue/83ca8ca5`).
- A new worktree's build cache is seeded before a gated cargo-ci. A cargo or test run is killed only when hung, and its orphans by working directory.
- A jit binary changes on this host in one step with the store change it needs, at a moment the steward names. The installed binary reads machine-local gate run records and ends a timed-out checker with SIGTERM, a grace period, then SIGKILL.
- `9a1b24fc` branches A and B land together on B's head with one gate set.

## What to do next

1. Read the window's result: record the `@/issue/3461a52a` curves on the issue with the executable digest, host and start loads, and read the quiet-host baseline.
2. Decide the build-loop levers with the owner from the report in the gf2 data directory, then dispatch `@/issue/a3e4caa2`.
3. Reword the records of `@/issue/66fefd9e` and `@/issue/506f76d4`, or land their branches: a clone of `origin` with the tracker mounted reports 68 reference faults for paths that exist only on those branches.
4. Take the next jit install when the jit project announces it: claims, leases and locks move out of the code repository's `.git` (`@/issue/15f56b64`), with a migration of the live state.
5. Resume the defect line with `@/issue/83ca8ca5` first, then `@/issue/a9ddca84`, `@/issue/d1eba151`, `@/issue/1be3446b`, `@/issue/581481c0`.
6. Resume the paused streams on the owner's word: `b4b4b9ee`, `d77176e5` with `ae03bcd0`, and the parked `45ecf975` branches.

## Traps

- Commit IDs older than 2026-11-17 in memory, logs and external notes are pre-rewrite IDs. Tracked text and tracker records are rewritten; the old-ID check of the rewrite tools reports none left, with two recorded false positives.
- A `git fetch` or push from a clone made before the rewrite brings the old history back. Re-clone, or reset to `origin/main`.
- The cargo token pool refills only when no client holds its lease. A stall at low load is a drained pool: check the pool log's last refill line before waiting for a timeout.
- The fast tier's 60 s suite budget ends a run with tests not run and none failed; `@/issue/959ab809` holds the captures.
- `@/issue/b8157e14` REQ-03 has no window line; its measurements need a harness command first.
- Earlier traps in the lead handoffs still bind: commit subjects, zsh word splitting, one landing on `main` at a time.

## Open questions for the owner

- When the rollback material may be deleted.
- Which build-loop levers to take, and a quiet-host hour for their verification.
- When the paused streams resume, and with how many leads.
- `@/issue/581481c0`: when to reconcile the profile package sources with the live state.
- Declared inputs for the `code-review` gate, so a verdict survives a merge of unrelated files.
- `41517289`, filing the amdgpu report upstream, stays a deferred owner action.

## References

- Charter: `dev/research/2caf738d-charter.md`.
- Rewrite runbook: `dev/active/b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md`.
- Landing flow: `dev/index.md`.

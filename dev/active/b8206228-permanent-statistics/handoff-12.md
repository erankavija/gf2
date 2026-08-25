# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 16

**Date:** 2026-08-25
**Session number:** 16
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — backlog; epic gates pending until all work items close.
- Wave in progress: wave 8 of 13. The frontier is `7a816262` alone; `1947125e` closed this session, and `679cf170` was closed externally by `agent:gf2-a1` (closure `51e28a31`).
- Active claims: `7a816262` — `agent:codex`, claimed 2026-08-17; doc-review and research-review pending; rework count at MAX.
- Open escalations: none. The session-16 dual-row gpu_hip escalation is owner-resolved and recorded in `progress.json`.
- Premeasurement: 1007 completed + 1 censored (position 539) valid, 432 pending (48 q5, 384 q7). Timers `gf2-b8206228-premeasure-frozen` (02:00) and `gf2-b8206228-premeasure-collect` (07:40) are armed for 2026-08-26.
- Progress file: `dev/active/b8206228-permanent-statistics/progress.json`, updated this session.

## What just happened

- Ran `jit recover` (one stale claims lock removed). Confirmed both 2026-08-25 timers fired: the resume session ran to its 19,800 s cap (positions 540–1007), the collector exited 7 (valid but incomplete).
- Diagnosed the 359 `data_row_count_2` positions: the frozen harness grid holds two gpu_hip specs per cell (`GPU_BATCHES=[256,1024]`) and the runner's `--batch-size` override precedes `--only` filtering, so every gpu_hip process measures its cell twice. Verified in the frozen source at `1350d5b4`.
- Escalated; owner decided: collector accepts dual-row gpu_hip scratch, selects the native-M (second) data row, first row preserved as unused replicate; bounded lead-direct repair; re-arm tonight.
- Landed the repair on main at `d7493bdc` (parser + premeasure test t11 + deviation record `dev/benchmarks/permanent_campaign/premeasure-v1-deviations.md`, linked to `7a816262`). Cherry-picked onto the pinned collector branch `worktree-agent-389aa4de` at `8811d7b6`; both test suites pass (11/11, 12/12).
- Discovered a main-side collect run mass-invalidates the cohort (see Traps); re-ran the collector from the amended pinned worktree — ledger now 1008 valid terminal rows, 432 pending, zero invalid.
- Closed `1947125e`: ran cargo-ci on the merged tree (passed at `51e28a31`), six-tier lead review PASS, closure commit `370eb43a`.
- Re-armed both transient timers for 2026-08-26 (logs `premeasure-resume-20260826.log`, `premeasure-collect-20260826.log` + `.exit.status` under the durable root).

## What to do next

- [ ] After 07:40 EEST 2026-08-26, inspect both service statuses and the dated logs. Collector exit 75 = still running, 0 = complete, 7 = valid but incomplete. If incomplete, re-arm the same root and run ID for another bounded night; ~432 q5/q7 positions were pending at arming and the previous session covered 468 positions in one cap.
- [ ] When collection reports complete: continue `7a816262` — selection ledger and frozen manifest generation/verification, then doc-review, research-review, and the six-tier lead review. Rework counter is at MAX: any review failure escalates; no unapproved repair.
- [ ] After `7a816262` closes, dispatch wave 9: `3f664839` and `73317b2e` (both DAG-blocked on it; verified this session). Re-read their live descriptions first.
- [ ] Keep `.agents/worktrees/agent-a39bb161` (frozen runner, `1350d5b4`) and `.agents/worktrees/agent-389aa4de` (collector branch at `8811d7b6`) intact while timers are armed.
- [ ] Report to the owner: isolated issue `c0bb2ab1` (outside this epic) fails repo-wide `jit validate`; epic-level gates that run `jit validate` will trip on it.

## Traps — do not repeat these

- **Do NOT run `premeasure-collect` for the premeasure-v1 cohort from current main.** `679cf170` renamed session provenance keys (`source_revision` → `repository_revision` and related), so main's collector marks all 1008 terminal positions `session_repository_revision_count_or_value` invalid. Evidence: this session's main-side rerun invalidated the whole cohort; re-running from the pinned collector branch (`worktree-agent-389aa4de`, `8811d7b6`) restored it. Receipts are never touched; only the regenerated ledger/candidates CSVs are affected.
- **Do NOT treat a 2-row gpu_hip scratch as corrupt or re-runnable.** It is the frozen harness's deterministic dual-spec behaviour; the receipt is final and the runner will (correctly) skip the position. The native-M second row is the measurement per the recorded owner decision; see `premeasure-v1-deviations.md` D-01.
- **Do NOT diagnose jit state from a subdirectory or worktree cwd.** A `cd` into a worktree subdir made `jit gate status-all` report passed gates as never-run and `.jit`/`.git/jit` as missing, mimicking the recorded "gate pass without persisted record" pitfall. Verify `pwd` is the repo root before concluding store corruption.
- **Do NOT re-derive the timer arming from `systemctl` after firing.** Transient units vanish once elapsed; the authoritative wrapper invocation is in the durable root's `sessions/*.provenance.txt` (`wrapper_invocation:` line).
- All unresolved traps from prior sessions remain in force, including the 02:00 device-work directive, the pristine-main overnight pre-flight, and the pinned-worktree preservation rules.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze issue: `jit issue show 7a816262`
- Deviation record: `dev/benchmarks/permanent_campaign/premeasure-v1-deviations.md`
- Premeasure plan: `dev/benchmarks/permanent_campaign/premeasure-plan-v1.md` (+ `.csv`)
- Reconciled progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Durable campaign root: `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`
- Tonight's logs (after firing): `premeasure-resume-20260826.log`, `premeasure-collect-20260826.log`, `premeasure-collect-20260826.exit.status` under the durable root
- Relevant commits: `370eb43a` (1947125e closure), `d7493bdc` (main repair), `8811d7b6` (pinned collector branch repair), `51e28a31` (679cf170 closure by gf2-a1)

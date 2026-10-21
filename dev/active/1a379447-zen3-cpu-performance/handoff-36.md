# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 40

**Date:** 2026-10-21
**Session number:** 40
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-35.md` in this directory

## Current state

- Epic: `1a379447`, state backlog in the tracker (derived; work is in progress)
- Wave in progress: wave 4 of 7 (story `ed3d490e` runs its own layers inside it)
- Children summary (label `epic:zen3-cpu-performance`): 171 done, 1 in_progress (`6230e0b6`), 4 backlog (epic included: `ed3d490e`, `e5f8b3ae`, `1362381c`), 27 rejected.
- Active claims: `6230e0b6` (agent:worker; its worktree is reclaimed and its work is on main), plus the epic itself.
- Open escalations: `6230e0b6` doc-review F1 placement; story `ed3d490e` criteria. Both are below and in `progress.json` `escalations`.
- Session rule (invoker, 2026-10-20, session 40): do not stop to ask. A task needing input is blocked and its question recorded. Write the handoff when no work remains that does not wait for input. Check workers every 30 minutes.
- Progress file: `progress.json` in this directory

## What just happened

- The invoker ruled on both session-39 questions. The `37aa7254` pilot window ran in-session. `262ca018` was approved with REQ-03 unchanged.
- The pilot's first window run (19:35Z) overlapped the lead's survey-test timing loop. The lead read its verdict line and then deleted it, which is a protocol.md:153 breach. Invoker ruling: preserve and disclose. It is preserved in `dev/bench_results/ed3d490e/decode-selector-holdout/37aa7254-...-pilot-superseded/` and recorded in `67517a7b/superseded-pilot-attempt.md`. Its ledger line is byte-identical to the rerun's, so the P-20 bound is unchanged.
- Done, all gates on main:
  - `67517a7b`: pilot collected; confirmation frozen and queued.
  - `262ca018`: survey tests and self-tests run in cargo-ci. `test_make_family.py` went from 110 s to 10 s through per-process lookup caching.
  - `7695aaee`: confirmation measured in-session (job `20a7f86392f16b4d`). Accepted, qualifies; the selectors of the committed envelope are retained.
  - `7211ae53`: `make-outcome-tables.py` and `dev/bench_results/ed3d490e/tables.md`. Rework 1 fixed peak memory projected across configurations and moved the selector family into its own part. Lead docstring trim `7da4f21e1`.
- `6230e0b6` (`findings.md`): rework 1 fixed doc-review F2–F4, and the lead fixed F5 in `c64a3efb9` (unreviewed). F1 remains blocking and is escalated. research-review passes. Verdicts are in `reviews/`.
- Filed, outside the epic: `d06a99ed` (worker-aware lane route, R-01) and `f768c925` (fixed-wave cost, R-04), both depending on `6230e0b6`; `c12f5b5d` (the frozen-fact replacement test hits the 15 s kill under load and leaks a fixture), depending on `67517a7b`.
- Reclaimed: `agent-262ca018`, `agent-7211ae53`, `agent-6230e0b6`. `agent-ed3d490e-run` is kept, on branch `worktree-agent-7695aaee` (merged) and clean.

## What to do next

- [ ] On the F1 ruling: apply it, then re-run `6230e0b6`'s doc-review (F5 included; `c64a3efb9` is unreviewed) and research-review. cargo-ci passed on `0c03a7a75`, and the later commit is docs only, but re-run it with the gates. Complete it with a round-2 verdict in `reviews/6230e0b6-r1.md`.
- [ ] On the story-criteria ruling: wire `d06a99ed` (and/or `f768c925`) into story `ed3d490e` as blockers and add the epic label, or record acceptance. Then reconcile `surfaced_pitfalls` against the story criteria and run the story checkpoint gates.
- [ ] After `ed3d490e`: `e5f8b3ae`, then `1362381c`, then epic completion (Section 10).

## Traps — do not repeat these

Traps of `handoff-35.md` and earlier remain in force.

- **Do NOT run anything CPU-heavy while a window job runs, the lead's own checks included.** The lead started the survey-test timing loop in the same turn as the window, and the pilot measured on a contended host. Start a window with a load guard (`/proc/loadavg` < 2 and no `rustc`/`cargo-ci.sh` in `ps -eo comm`), then wait for its exit.
- **Do NOT delete a contaminated run after reading its verdict.** protocol.md:153 makes a read attempt unvoidable. Stop the window before reading output, or preserve the run as superseded.
- **Do NOT gate a wait loop on `pgrep -f`.** The hook blocks it, because the pattern matches the loop's own shell. Use `ps -eo comm`.
- **Do NOT file a follow-up with no dependency edge.** `jit validate` fails on isolated issues. Add an edge to the issue that surfaced it, and keep the epic label off until the invoker brings it into scope.
- **Check a lead commit's subject length before committing.** `c64a3efb9` had to be amended from 75 characters.
- **`jit doc add` refuses a binary file** ("not UTF-8"). Pass `--skip-scan` for `.tar.zst` evidence.
- **The `protocol_contracts` frozen-fact replacement test TIMEOUTs under load and leaks a fixture** (`c12f5b5d`). Retry once with `BUSY_HOST_OVERRIDE=1` and read nothing else into it.

## Open questions needing invoker input

- Question: how to resolve `6230e0b6` doc-review F1, the outcome record's location?
  - Context: REQ-01 names `dev/active/ed3d490e-zen3-ldpc-frame-simd/findings.md`. `@/inv/active-document-layout` requires `dev/active/<epic-short-id>-<slug>/`. `jit doc dir ed3d490e dev/active` resolves to the story directory, where the story's plan, calibration and survey already live.
  - Options: A amend the invariant to cover breakable containers (epic or story short id); B amend REQ-01 and move the record to `dev/active/1a379447-zen3-cpu-performance/6230e0b6/findings.md`; C move the whole story directory under the epic.
  - Recommendation: A, because it matches `jit doc dir` and touches no evidence paths.
- Question: what do story `ed3d490e` REQ-01 (adoption), REQ-10 and REQ-13, recorded as open or PARTLY MET in `findings.md`, require before the story closes?
  - Context: The committed envelope opens the lane route at every worker count, but calibration and holdout measured one worker, and the multicore confirmation records the lane route failing at 6+ workers (DVB-T2) and 24 logical CPUs (NR) (R-01, `d06a99ed`). The AFF3CT INTRA/INTER modes have only the one-worker exploratory pilot, and AFF3CT batch fill and workspace are unavailable (REQ-10). The cost of fixed waves is unmeasured (R-04, `f768c925`).
  - Options: A bring `d06a99ed` into the story as a blocker, accept REQ-10's unavailable cells as recorded, and keep `f768c925` outside; B accept all gaps as recorded and close with REQ-10/REQ-13 partly met; C bring both follow-ups in and require further AFF3CT cells.
  - Recommendation: A. R-01 is a production default that the evidence contradicts.

## Reference artefacts

- Collections: `67517a7b/{collection.md,superseded-pilot-attempt.md}`, `7695aaee/{collection.md,outcome.md}`
- Tables: `dev/bench_results/ed3d490e/tables.md`; record: `dev/active/ed3d490e-zen3-ldpc-frame-simd/findings.md`
- Reviews: `reviews/{67517a7b,262ca018,7695aaee,7211ae53,6230e0b6}-r1.md`

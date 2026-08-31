# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 33

**Date:** 2026-08-31
**Session number:** 33
**Prior handoffs:** `handoff.md` through `handoff-28.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic: `b8206228` — state: backlog; wave 10 of 13 is in progress.
- Direct dependencies: 11 done, 4 backlog (`0665e7be`, `53d8e438`, `b05c908b`, `b5d45bcd`). The operational frontier is the non-direct prerequisite `02b8137c`, which is in progress.
- Active claims: `b8206228` and `02b8137c` are assigned to `agent:jit-execution-lead`; no sub-agent or worker worktree remains active.
- Open escalation: `02b8137c` needs an owner decision on how the public validation journal and final receipt encode an unavoidable old-producer/new-producer boundary without redrawing or overwriting evidence.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics/` reflects this state.
- Campaign state: no campaign-purpose draw occurred. `ed494117`, `1d0b3ec4`, and `90a61cd4` remain blocked on `02b8137c`; the prior separate-go requirement remains in force after validation closes.

## What just happened

- Confirmed `gf2-predraw-validation2.service` started at 2026-08-31 02:00:30 and ran for 17m36s; timer or user-bus permission was not the failure.
- Preserved the complete failed-run journal at commit `3fd2893b`; it contains five immutable passing terminals: q=3,n=1..4 and q=5,n=1. No terminal was changed, deleted, or redrawn.
- Reproduced the publisher failure in an exact Rust probe: default serde_json 1.0.151 parsing shifts q=5 `log_p_value` bits `bfb87fcfb6c9e338` to `bfb87fcfb6c9e339` and `p_value` bits `3fed145e4cad104e` to `3fed145e4cad104d`.
- Disproved the racing-writer hypothesis: the publisher hardlinked the intended bytes, then rejected its own deserialized value because of the one-ULP shifts.
- Dispatched high-tier isolated rework. Commit `1acd2a3b` enables serde_json `float_roundtrip` for `gf2-sim` and adds bit-exact q=5 publication, committed-terminal adoption, and tamper-rejection regressions.
- Lead-reviewed and merged the repair at `2e760af7`; full `permanent_validation` release tests passed 25/25, and integrated `./scripts/cargo-ci.sh` passed 5,199 tests with zero failures plus check, Clippy, formatting, tuning, and baked-artifact stages.
- Reclaimed the worker worktree after merge and cache harvest. Main is the only active checkout for this issue.

## What to do next

- [ ] Obtain the owner's choice on the `02b8137c` provenance-continuation escalation below. Do not run the validator before that decision.
- [ ] If the recommended continuation is authorized, dispatch a high-tier implementation worker for a narrowly versioned migration/continuation record: preserve the original run-state and five terminal digests, bind both exact producer identities and their boundary, admit only the five missing addresses, and report both producer segments in the final receipt.
- [ ] Independently review the continuation schema and tests, merge it, then run the complete repository CI contract on the integrated tree.
- [ ] Run only the five missing validation anchors in the owner-approved post-02:00 window; verify and commit the final receipt plus complete journal evidence.
- [ ] Link the receipt to `02b8137c`, evaluate all four configured gates, perform the six-tier lead review, and close the issue only if every hard criterion is met.
- [ ] Ask the owner for the already-required separate campaign go. If authorized, execute exact q=7,n=20 to terminal state before scheduling any other campaign-purpose cell; then serialize the remaining q=7, q=5, and q=3 arms according to the frozen coordinator protocol.
- [ ] Continue through `f27150a5`, wave 12, wave 13, and the epic gates only after the campaign arms and downstream artifacts close.

## Traps — do not repeat these

- **Do not diagnose this as a permission or timer-start problem.** The service ran for 17m36s and consumed 6h26m CPU. Sandboxed `systemctl --user` inspection needs the explicit user bus or host approval, but that inspection constraint did not prevent the timer from firing.
- **Do not diagnose a racing or second writer.** An exact Rust probe reproduced both one-ULP shifts using a single deserialize. The hardlinked q=5 terminal bytes are the intended immutable bytes.
- **Do not assume ordinary JSON finite-float parsing preserves Rust `f64` bits.** In this dependency configuration, default serde_json changed the two q=5 values. The merged `float_roundtrip` feature and regressions are the required repair.
- **Do not resume with the old binary.** Its post-publication verifier deterministically rejects the already committed q=5 terminal, so it cannot adopt the journal and reach the remaining anchors.
- **Do not relabel the repaired binary as the original producer or loosen runtime equality silently.** `run-state.json` binds revision `397520d6...`, binary SHA-256 `76c778f1...`, dependency identity, invocation, and worker count. Hiding the transition violates runtime-observed provenance.
- **Do not overwrite, delete, or redraw any of the five terminal addresses.** The journal is committed at `3fd2893b`; the no-redraw and falsification-preservation contracts apply even though the final receipt was not produced.
- **Do not start a campaign-purpose draw after validation closes without a separate owner go.** The existing authority covered arming validation only. Exact q=7,n=20 must become terminal before any other campaign-purpose cell is scheduled.
- **Do not treat progress wave 10's grouped arms as permission to run them in parallel.** The operative protocol serializes q=7,n=20 first, then the remainder of q=7, then q=5 and q=3.

## Open questions needing invoker input

- Question: How should `02b8137c` continue across the unavoidable producer-identity boundary?
  - Context: Five valid terminals are immutably bound to the original producer; the remaining five can only be produced by the repaired, CI-clean binary.
  - Options: (A) authorize a versioned provenance-preserving continuation binding both producer identities, the five-terminal boundary, and all terminal digests, with no redraw; (B) authorize a newly preregistered validation cohort and preserve this attempt as falsification evidence, repeating all anchors under a changed cohort contract; (C) stop and leave the validation issue, campaign arms, and epic blocked.
  - Recommendation: Option A. It preserves every observation and the preregistered scientific addresses, makes the implementation transition explicit, and performs neither overwrite nor redraw.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Validation issue: `jit issue show 02b8137c`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior state: `dev/active/b8206228-permanent-statistics/handoff-28.md`
- Preserved journal: `dev/active/02b8137c/validation-journal/`
- Original run-state: `dev/active/02b8137c/validation-journal/run-state.json`
- Q=5 terminal that exposed the defect: `dev/active/02b8137c/validation-journal/q5-n01-s0.terminal.json`
- Preregistration: `dev/active/02b8137c/pre-draw-validation-v1-preregistration.json` and `.md`
- Journal preservation commit: `3fd2893b`
- Repair implementation: `1acd2a3b`
- Integrated repair merge: `2e760af7`
- Runbook and log: `/home/vkaskivuo/.local/state/gf2-validation/run-validation-0200.sh` and `validation-run.log`

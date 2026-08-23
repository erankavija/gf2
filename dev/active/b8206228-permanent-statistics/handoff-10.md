# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 14

**Date:** 2026-08-24
**Session number:** 14
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog.
- Wave in progress: wave 7 of 13, with the long-running wave-5 freeze `7a816262` also in progress after crash recovery.
- Children summary for `epic:permanent-statistics`: 61 done, 2 in progress, 14 backlog, 0 ready/rejected.
- Active claims: `7a816262` — `agent:codex-luna`, claimed 2026-08-17; `a39bb161` — `agent:codex-luna`, claimed 2026-08-20.
- Open escalations: `a39bb161` is at rework count 2 (MAX) and has a new independent-review failure; owner decision required before repair.
- Progress file: `progress.json` in the epic artifact directory, `jit doc dir b8206228 dev/active` (reflects the above).

## What just happened

- Recovered after the host crash with `jit recover` clean and `jit validate` passing; no surviving premeasure process receipt or campaign shard was credited.
- Audited the failed 2026-08-19 timer: it exited immediately with infrastructure status 2; all 1,440 preregistered processes therefore restart as a fresh `premeasure-v1` cohort.
- Created durable recovery root `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`; fresh prepare and measurement have not run.
- Recycled `.agents/worktrees/agent-a39bb161` for difficult rework-2; worker commit `7316c062` converged the public field API on `AcceleratorCostTable`, added a two-cell behavioural test, and completed the Rustdoc without GPU work.
- Leak-check passed; merged the worker as `f077c380`; retained the clean worktree for reuse as requested.
- Independent review failed `a39bb161`: `permanent_campaign` checks the whole manifest when deciding whether `--accelerator-cost-table` is mandatory, although `--q` executes only one field. A mixed manifest therefore rejects a selected processor-only field because another field uses an accelerator.
- Rework count `a39bb161 = 2` is already MAX; recorded a pending escalation instead of performing an unapproved third repair.
- Audited future pending work: `53d8e438` has a pre-data contract mismatch between REQ-03's unspecified standard-error multiple and the protocol's interval-aware delta-AIC rule; no other missing decision or dependency edge was found.
- Preserved unrelated live `.jit` modifications for `312200e4`; they remain unstaged and uncommitted by this session.

## What to do next

- [ ] Resolve the `a39bb161` escalation. Recommended owner decision: approve one bounded lead-direct repair that scopes the CLI cost-table requirement to the selected field and adds a real-binary mixed-manifest regression.
- [ ] After approval, apply the repair in the retained worktree, leak-check, merge, then rerun the merged-tree `cargo-ci` and `code-review` gates and confirm persisted gate status before six-tier closure.
- [ ] Do no GPU-heavy work before 02:00 local. At or after 02:00 the owner guarantees the device is safe unless they say otherwise; do not depend on cross-user process visibility.
- [ ] Once `a39bb161` is reviewed and main is clean, run fresh campaign `prepare` with `CAMPAIGN_TARGET_ROOT=/data/gf2-campaigns/b8206228/premeasure-recovery-20260824` and stable `CAMPAIGN_RUN_ID=premeasure-v1`, verify all 120 rows against the real binary's admission path, then run `premeasure --session-cap 19800` under the full-host lock.
- [ ] Persist raw premeasure state outside ignored `target/`; collect and resume from the durable root until all 1,440 processes have terminal receipts, then finish `7a816262` phase 3 and its doc/research gates.
- [ ] Before dispatching `53d8e438`, obtain owner approval either to amend REQ-03 to the protocol's interval-aware delta-AIC rule (recommended) or to preregister a concrete standard-error multiple before data exist.

## Traps — do not repeat these

- **Do not treat a selected-field execution as manifest-global.** `crates/gf2-sim/src/bin/permanent_campaign.rs:150` requires the cost table if any manifest field has accelerator cells, while the binary selects one field later; independent review demonstrated that this rejects a valid processor-only selected field. Scope admission requirements to `--q` and test the real binary with a mixed manifest.
- **Do not infer another user's game state from process visibility.** The owner's Baldur's Gate 3 executable is `/data/SteamLibrary/steamapps/common/Baldurs Gate 3/bin/bg3`, but it runs as another user. The authoritative admission rule is the owner's guaranteed 02:00-local safe window unless they say otherwise.
- **Do not keep measurement recovery state only under ignored `target/`.** The crash erased binaries, receipts, logs, and provenance there. Use the durable `/data/gf2-campaigns/...` root and commit versioned evidence at the protocol's checkpoints.
- **Do not commit dispatch or rework prompts.** The owner explicitly ruled that prompts are ephemeral coordination material. The accidental session-14 prompt was unlinked and deleted; future dispatches stay outside the repository.
- **Do not absorb unrelated shared JIT materialization.** `.jit/events.jsonl` and `.jit/issues/312200e4-dce9-44d3-8b55-c20c610e1c03.json` belong to another participant; stage only explicit epic handoff/progress paths until those changes are resolved.
- All unresolved traps from sessions 3–12 remain in force; start with `handoff-9.md` and follow its prior-handoff chain.

## Open questions needing invoker input

- Question: May the lead perform one bounded direct repair of `a39bb161` beyond the normal two-rework maximum?
  - Context: independent review found a new field-scope CLI defect after rework-2; the scheduler and public API findings are otherwise closed.
  - Options: approve field-scoped CLI repair plus real-binary regression and gate rerun; take over manually; reject the issue and block its dependents.
  - Recommendation: approve the bounded lead-direct repair because it is local, preserves the reviewed per-cell contract, and directly tests the remaining failure.
- Question: Which preregistered decision rule should govern `53d8e438` REQ-03?
  - Context: the criterion requires an unspecified multiple of standard error, but the protocol already preregisters interval-aware delta-AIC and prohibits post-data threshold selection.
  - Options: amend REQ-03 to interval-aware delta-AIC; preregister a concrete standard-error multiple before campaign data; reject/defer the issue.
  - Recommendation: amend REQ-03 to the protocol rule so the criterion and existing preregistration have one source of truth.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Blocking issue: `jit issue show a39bb161`
- Long-running freeze: `jit issue show 7a816262`
- Progress and escalation record: `dev/active/b8206228-permanent-statistics/progress.json`
- Frozen-manifest plan: `dev/active/b8206228-permanent-statistics/premeasure-plan-v1.md`
- Prior recovery handoff: `dev/active/b8206228-permanent-statistics/handoff-9.md`
- Accelerator repair commits: `7316c062` (worker), `f077c380` (merge)
- Durable campaign root: `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`
- Protocol: `dev/simulation_results/permanent-zero-fraction/protocol.md` section Backend freeze

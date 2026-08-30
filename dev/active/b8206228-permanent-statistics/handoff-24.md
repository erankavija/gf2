# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 28

**Date:** 2026-08-30
**Session number:** 28
**Prior handoffs:** `handoff.md` through `handoff-23.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic: `b8206228` — state: backlog.
- Wave in progress: wave 10 of 13.
- Children summary: 11 direct dependencies done, 4 backlog; the active deeper frontier has three ready campaign arms, one blocked finalizer, and one in-progress importance-sampling issue.
- Active claims: None. `6639435f` remains in the JIT `in_progress` state, but its expired lease was recovered and its clean worktree is deliberately unmerged.
- Open escalations: `6639435f` awaits owner authorization for one bounded high-tier rework cycle covering its four final publication/lifecycle findings.
- Progress file: `progress.json` in the epic's artifact directory, `jit doc dir b8206228 dev/active` (reflects the above).
- No campaign, Cargo, nextest, device, ignored-test, benchmark, or measurement process is running. The frozen campaign directory contains only `manifest.json`, `checksums.sha256`, and `freeze.md`.

## What just happened

- Reconciled the live JIT DAG, progress record, git tree, frozen campaign directory, and agent results in response to the owner's graceful-stop instruction.
- Confirmed `73317b2e` is done on main at merge `35da7951`, with registered `cargo-ci` and `code-review` gates passed.
- Confirmed the final scoped implementation review and exact-refreeze audit passed with no findings. The earlier broad FAIL was the diagnosis that drove the completed repair; it is not the final issue verdict.
- Confirmed the canonical exact-cell cutover remains intact: no q-only compatibility path, no direct-emitter bypass, and no campaign-specific frozen identity embedded in the reusable `gf2-sim` coordinator API.
- Started no campaign arm and produced no draw, checkpoint, receipt, summary, or sidecar.

## What to do next

- [ ] Resolve the owner escalation on `6639435f`; the lead recommends the single bounded high-tier repair covering all four findings, followed by fresh independent review and all configured gates.
- [ ] When epic execution resumes, claim `ed494117` first and run exact $(q,n)=(7,20)$ through the canonical coordinator. Do not schedule any other campaign-purpose cell until it is terminal.
- [ ] Complete the remaining $q=7$ cells, then the ready $q=5$ and $q=3$ arms, preserving durable checkpoints and canonical projections.
- [ ] Complete `f27150a5` only after all three field arms are terminal, then continue the remaining analysis waves in dependency order.
- [ ] Finish with the epic's `repo-validate`, `doc-review`, and `holistic-review` gates and a criterion-to-artifact completion audit.

## Traps — do not repeat these

- **Do not reopen the exact-cell emitter repair.** `73317b2e` is merged, gated, independently reviewed, and done; `handoff-23.md` records its identities and stopping boundary.
- **Do not treat the historical broad FAIL as the final verdict.** It predates the scoped repair. The final review and refreeze audit passed, and the registered gates are the authoritative completion evidence.
- **Do not parallelize the first campaign action.** Hard ordering requires exact $(7,20)$ to become terminal before any other campaign-purpose cell is scheduled; readiness of the $q=5$ and $q=3$ arms does not relax that order.
- **Do not reintroduce a superseded representation.** A q-only selector, path-only interpretation input, public sidecar mutator, or direct-emitter bypass violates canonical cutover.
- **Do not put campaign-specific paths or frozen digests into the reusable library.** `gf2-sim` owns the general coordinator; `permanent_campaign` remains the thin repository campaign consumer.
- **Do not merge `worktree-agent-6639435f` yet.** Its four owner-escalated boundary findings remain unresolved; see `handoff-22.md` and the pending progress-file escalation.

## Open questions needing invoker input

- Question: Should `6639435f` receive one bounded high-tier rework cycle covering all four final audit findings?
  - Context: Ordinary rework is exhausted; the unmerged branch otherwise blocks the thin production runner and downstream rare-event report.
  - Options: authorize the complete repair; narrow the artifact threat model and accept path-based TOCTOU risk; or leave the issue blocked.
  - Recommendation: Authorize the complete repair because the four findings jointly define the promised fail-closed immutable-publication boundary.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Progress and wave plan: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior stopping boundary: `dev/active/b8206228-permanent-statistics/handoff-23.md`
- Acceptance issue: `jit issue show 73317b2e`
- Canonical coordinator: `crates/gf2-sim/src/permanent_campaign/coordinator.rs`
- Thin campaign consumer: `crates/gf2-sim/src/bin/permanent_campaign.rs`
- Frozen campaign: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`
- Importance-sampling branch: `.agents/worktrees/agent-6639435f` at `49b4d452`

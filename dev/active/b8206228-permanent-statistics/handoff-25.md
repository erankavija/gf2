# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 29

**Date:** 2026-08-30
**Session number:** 29
**Prior handoffs:** `handoff.md` through `handoff-24.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic: `b8206228` — state: backlog.
- Wave in progress: wave 10 of 13.
- Children summary: 11 direct dependencies done, 4 backlog; campaign arms `ed494117`, `1d0b3ec4`, and `90a61cd4` remain ready, and `f27150a5` remains blocked on them.
- Active claims: None. The read-only q=7 admission auditor was stopped on the owner's handoff instruction.
- Open escalations: `6639435f` awaits owner authorization for one bounded high-tier rework cycle covering its four final publication/lifecycle findings.
- Progress file: `progress.json` in the epic's artifact directory, `jit doc dir b8206228 dev/active` (reflects the above).
- No campaign cell was executed. The frozen campaign directory still contains only `manifest.json`, `checksums.sha256`, and `freeze.md`.

## What just happened

- Reconciled `ed494117` and its hard criteria against the frozen protocol, manifest, coordinator, exact-cell binary, and prior handoffs.
- Found that a later shared-target Cargo build had overwritten `target/release/permanent_campaign`; its digest was `0b345f0c…`, not the frozen `c9b2307a…`, so it could not receive emission approval.
- Recreated the exact source checkout at detached revision `785e3146` in `.agents/worktrees/agent-73317b2e` and repeated the recorded Rust 1.95 HIP build command. The resulting emitter reproduced the frozen digest exactly: `c9b2307a9f1d3ae4fedd5c442bf1ba79786b27727f8b0d99eb96fea23e3c501d`.
- Preserved those exact bytes at `/home/vkaskivuo/.local/state/gf2-frozen/permanent_campaign-c9b2307a` and copied them to `target/release/permanent_campaign` for the canonical relative invocation.
- Ran only read-only/no-draw admission checks: supplied-emitter inspection passed, and the exact dry schedule returned `schedule q=7 n=20 shards=3` with the committed accelerator-cost table. It did not open the campaign output, coordinator receipt, execution lock, or sampler.

## What to do next

- [ ] Before any dispatch or execution, verify both preserved and target emitter bytes still hash to `c9b2307a9f1d3ae4fedd5c442bf1ba79786b27727f8b0d99eb96fea23e3c501d`; restore the target copy from the preserved file if a Cargo build replaced it.
- [ ] Claim `ed494117`, commit the JIT state transition, and dispatch one high-tier campaign worker. Keep Cargo, nextest, gates, benchmarks, and other campaign arms idle during measurement.
- [ ] From the main repository root, execute the frozen exact invocation for $(q,n)=(7,20)$ through the canonical coordinator. Do not schedule any other campaign-purpose cell until it is terminal.
- [ ] Review the terminal coordinator receipt and raw identities against `ed494117` before proceeding through the remaining exact $q=7$ cells.
- [ ] Resolve the separate `6639435f` owner escalation before merging its worktree.

## Traps — do not repeat these

- **Do not trust `target/release/permanent_campaign` after any Cargo build.** The shared target had already replaced the frozen emitter with digest `0b345f0c…`; hash immediately before execution and require `c9b2307a…`.
- **Do not rebuild the campaign emitter from current main.** The reproducible build is source revision `785e3146` with `cargo +1.95.0 build -p gf2-sim --release --features hip --bin permanent_campaign --bin permanent_dataset`; that exact reconstruction matched the frozen digest.
- **Do not execute from the detached reconstruction worktree.** Its source revision is correct for the binary but its checkout predates the committed manifest refreeze. Actual execution uses the committed campaign directory on main and the canonical relative `target/release/permanent_campaign` invocation.
- **Do not mistake the dry schedule for campaign progress.** It is expressly no-draw validation and created no receipt, lock, checkpoint, shard, summary, or sidecar.
- **Do not parallelize the first campaign action.** Exact $(7,20)$ must become terminal before another campaign-purpose cell is scheduled.
- **Do not run Cargo or gates concurrently with campaign measurement.** The shared target is serialized, and rebuilding can also replace the pinned executable between inspection and launch.
- **Do not reintroduce a q-only selector or another compatibility representation.** Canonical exact-cell cutover is complete.

## Open questions needing invoker input

- Question: Should `6639435f` receive one bounded high-tier rework cycle covering all four final audit findings?
  - Context: Ordinary rework is exhausted; its clean, unmerged branch otherwise blocks the thin production runner and downstream rare-event report.
  - Options: authorize the complete repair; narrow the artifact threat model and accept path-based TOCTOU risk; or leave the issue blocked.
  - Recommendation: Authorize the complete repair because the findings jointly define the promised fail-closed immutable-publication boundary.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Wave state: `dev/active/b8206228-permanent-statistics/progress.json`
- q=7 issue: `jit issue show ed494117`
- Frozen protocol: `dev/simulation_results/permanent-zero-fraction/protocol.md`
- Frozen campaign: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`
- Exact refreeze record: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/freeze.md`
- Preserved emitter: `/home/vkaskivuo/.local/state/gf2-frozen/permanent_campaign-c9b2307a`
- Exact-source reconstruction: `.agents/worktrees/agent-73317b2e` at `785e3146`
- Canonical coordinator: `crates/gf2-sim/src/permanent_campaign/coordinator.rs`
- Importance-sampling branch: `.agents/worktrees/agent-6639435f` at `49b4d452`

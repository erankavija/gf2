# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 27

**Date:** 2026-08-30
**Session number:** 27
**Prior handoffs:** `handoff.md` through `handoff-22.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic `b8206228` remains backlog at 11/15 direct dependencies done (73%). Its four open direct dependencies are still downstream of campaign execution, finalization, analysis, or the separately escalated importance-sampling work.
- Issue `73317b2e` is done. Main includes merge `35da7951`; its registered `cargo-ci` and `code-review` gates pass.
- Wave 10 is the active frontier. Campaign arms `ed494117` ($q=7$), `1d0b3ec4` ($q=5$), and `90a61cd4` ($q=3$ reproduction) are ready and unassigned. None was dispatched in this session.
- Deterministic finalization `f27150a5` remains blocked on all three campaign arms.
- Importance-sampling issue `6639435f` remains in progress and unmerged at its owner-escalation boundary described in `handoff-22.md`; this session did not resume or alter it.
- The frozen campaign directory contains no shard, checkpoint, coordinator receipt, summary, or derived sidecar. No campaign, device, ignored-test, benchmark, or measurement run is active.

## What this session accomplished

- The production campaign path is one coordinator-owned exact-cell transaction. It persists authorization before sampling, binds the live executable, committed manifest, protocol, exact effective arguments, worker count, accelerator-cost input, and interpretation evidence, and keeps retry, terminalization, halt, and projection state resumable.
- The canonical selector is the exact pair `--q FIELD --n ORDER`. There is no q-only compatibility path. The frozen first invocation selects only $(q,n)=(7,20)$.
- Acceptance uses the shared exact-binomial decision path and the finite-$n$ determinant null. Rejection and exhausted mechanical retry halt later scheduling while preserving durable evidence.
- Field summaries and interpretation sidecars are derived only from the persisted coordinator receipt and canonical raw shards. The $q=3$ comparison validates source counts and canonical reported rounding; the $q=5$ and $q=7$ conditional claim is licensed by exact artifact identity.
- The reusable capability lives in the `gf2-sim` library. `permanent_campaign` is a thin repository campaign consumer that supplies the frozen evidence identities and command-line configuration; the library does not embed this campaign's paths or digests.
- The emitter was rebuilt with Rust 1.95 and refrozen without changing the 63-cell scientific payload, 14,229-shard inventory, or stream allocation. The manifest content digest is `1a22968435bf3bee958ef07204505d85827fb6227af6e2d2d1d864a0f7a93dc2`; the emitter digest is `c9b2307a9f1d3ae4fedd5c442bf1ba79786b27727f8b0d99eb96fea23e3c501d`.
- The coupled accelerator launch-cost validator and rendered receipt bind the refrozen manifest. Their underlying 15-cell cost CSV is unchanged.
- Independent implementation review and exact-refreeze audit passed with no remaining finding. The complete repository CI contract passed on the clean issue branch and again as the registered gate at the merge commit.

## Stopping boundary

This handoff is intentional. The current implementation and evidence work is complete, and no subsequent dependency wave is started. In particular, readiness of the three campaign-arm issues is not authorization to begin a draw during this session.

## Resume order

1. If campaign execution is authorized, claim `ed494117` first and make its first campaign-purpose action the exact $(7,20)$ cell through the canonical coordinator. Do not schedule any other cell until that cell is terminal.
2. Continue the remaining exact $q=7$ cells only as the persisted coordinator permits, then execute the ready $q=5$ and $q=3$ arms under their issue criteria. Preserve checkpoints and do not repeat completed work.
3. Complete `f27150a5` only after all three field arms are terminal and their canonical projections exist.
4. Resolve the separate owner escalation for `6639435f` before merging or extending its branch.
5. Proceed topologically through the remaining analysis dependencies, then run the epic's repository, documentation, and holistic gates.

## Do not regress

- Do not restore a q-only selector, path-only interpretation input, caller-owned sidecar mutator, direct-emitter bypass, or another compatibility representation. Canonical cutover is complete.
- Do not move campaign-specific paths, digests, or orchestration policy into the reusable library API.
- Do not edit the frozen scientific cell payload or reinterpret runtime Git context as the emitter source revision. The supplied-emitter check owns binary identity.
- Do not merge `worktree-agent-6639435f` while its four owner-escalated boundary findings remain unresolved.
- Do not run Cargo or nextest concurrently against the shared target, and do not opt into ignored tests during ordinary issue work.

## Reference points

- Closed acceptance issue: `jit issue status 73317b2e`
- Epic progress: `jit issue progress b8206228`
- Canonical coordinator: `crates/gf2-sim/src/permanent_campaign/coordinator.rs`
- Thin campaign consumer: `crates/gf2-sim/src/bin/permanent_campaign.rs`
- Frozen campaign: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`
- Exact refreeze record: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/freeze.md`
- Execution-lead state: `dev/active/b8206228-permanent-statistics/progress.json`

# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 30

**Date:** 2026-08-30
**Session number:** 30
**Prior handoffs:** `handoff.md` through `handoff-25.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic: `b8206228` — state: backlog.
- Wave in progress: wave 10 of 13.
- Children summary: 11 direct dependencies done and 4 backlog. New prerequisite `02b8137c` is ready; campaign arms `ed494117`, `1d0b3ec4`, and `90a61cd4` are blocked on it; `f27150a5` remains blocked on all three arms.
- Active claims: no running validation or campaign worker. `02b8137c` is unassigned. `6639435f` remains assigned in JIT to `agent:codex-sol-importance`, but its clean review branch is idle and unmerged pending the escalation below.
- Open escalations: `6639435f` awaits owner authorization for one bounded high-tier rework cycle covering its four final publication/lifecycle findings.
- Progress file: `progress.json` in the epic's artifact directory, `jit doc dir b8206228 dev/active` (reflects the above).
- No Cargo, nextest, validation, campaign, device, benchmark, or measurement process is running. No validation or campaign-purpose draw occurred. The frozen campaign directory still contains only `manifest.json`, `checksums.sha256`, and `freeze.md`.

## What just happened

- Audited the frozen protocol's complete “Validation before campaign draws” section before launching the exact $(7,20)$ cell.
- Proved that completed issue `44534b2f` is insufficient launch evidence: its shared test uses 4,096 draws and Clopper–Pearson coverage, while the frozen protocol requires ten anchors, 1,024-matrix replay, exactly 400,000 validation-purpose draws per anchor, and a probability-ordering exact two-sided test at 0.001.
- Created `02b8137c` with hard criteria for independent exhaustive enumeration, production permanent/pooling and determinant agreement, sampler replay, strict exact-test decisions, immutable runtime-observed receipt provenance, a public reusable validation API, focused release tests, and all four configured gates.
- Wired `ed494117`, `1d0b3ec4`, and `90a61cd4` behind `02b8137c`; no campaign arm can start until the validation issue is done.
- Dispatched a high-tier implementation worker in `.agents/worktrees/agent-02b8137c`, then stopped it on the owner's handoff instruction. The branch/worktree is clean at `89c7e8cc`; it has no commits, modifications, or untracked files and ran no Cargo command, test, validation draw, or campaign draw.
- Released `02b8137c` from the stopped worker. Its research identified `gf2-stats::binomial::two_sided_test`, `ExactBinomialTest::rejects_at`, the canonical validation sampler/address types, and the production schedule evaluator/pooling path as the implementation seams.

## What to do next

- [ ] Claim `02b8137c` with a high-tier implementation worker and resume from the clean isolated worktree, or recreate it from current main if main has advanced.
- [ ] Inspect `schedule.rs` evaluation/pooling and `schema.rs` backend support, then add failing release-mode behavioral tests for 1,024-draw replay, equality-at-0.001 failure, failure preservation/no redraw, stream-purpose separation, and receipt round-trip.
- [ ] Implement one public reusable validation API in `gf2-sim` that reuses the production evaluator/pooling/determinant path. Keep the repository runner thin and campaign constants outside the library.
- [ ] Commit the preregistration, schema, constants, tests, and implementation before the first fixed validation draw. Then build with Rust 1.95 release and execute the complete 400,000-draw-per-anchor run after 02:00 local under `dev/scripts/ccx1-bench-flock.sh --full-host`.
- [ ] Preserve the first validation outcome without redraw, link its immutable receipt to `02b8137c`, run `cargo-ci`, `code-review`, `doc-review`, and `research-review`, complete lead review, and close the issue only if every hard criterion and gate passes.
- [ ] After validation closes, restore `target/release/permanent_campaign` from `/home/vkaskivuo/.local/state/gf2-frozen/permanent_campaign-c9b2307a`, verify SHA-256 `c9b2307a9f1d3ae4fedd5c442bf1ba79786b27727f8b0d99eb96fea23e3c501d`, and execute exact $(q,n)=(7,20)$ to terminal state before any other campaign-purpose cell.
- [ ] Resolve the separate `6639435f` owner escalation before merging or extending its branch.

## Traps — do not repeat these

- **Do not launch from the successful dry schedule.** It proves exact selector admission only; the frozen protocol independently requires the complete ten-anchor validation receipt before any campaign-purpose draw.
- **Do not treat `44534b2f` as the frozen validation run.** `crates/gf2-algebra/tests/permanent_exact_anchors.rs` fixes 4,096 draws and Clopper–Pearson coverage, not the protocol's 1,024 replay plus 400,000-draw probability-ordering exact test.
- **Do not substitute the older order-3 study receipt.** `dev/studies/b488f02c/order3-anchor-2026-08-08.txt` covers only $n=3$ for three fields and uses a different statistical decision.
- **Do not duplicate the production evaluator as a private validation implementation.** Extend and reuse the `gf2-sim::permanent_campaign::schedule` evaluation, determinant, and pooling path through a public library API, preserving library-first generality and crate direction.
- **Do not use the production evaluator as its own independent oracle.** The exhaustive enumerator must visit every matrix exactly once independently; then compare the production path and each supported selectable backend against it.
- **Do not run the 400,000-draw phase before committing preregistration and producing code.** Source, constants, addresses, schema, exact rule, and no-redraw policy must predate the first validation draw.
- **Do not redraw a failed validation anchor.** Preserve the failure receipt and keep campaign execution blocked.
- **Do not use `StreamPurpose::CampaignCell` in validation or create campaign shards, coordinator receipts, checkpoints, summaries, or sidecars.** Validation and campaign streams/artifacts remain disjoint.
- **Do not trust `target/release/permanent_campaign` after Cargo work.** Rebuilds can replace the pinned bytes; restore from the preserved emitter and hash-check immediately before campaign execution.
- **Do not rebuild the frozen emitter from current main or execute from the detached reconstruction worktree.** The binary source revision is `785e3146`; actual campaign execution uses main's committed frozen campaign directory and the pinned relative executable.
- **Do not merge `worktree-agent-6639435f`.** Its four owner-escalated publication/lifecycle findings remain unresolved; see `handoff-22.md`.

## Open questions needing invoker input

- Question: Should `6639435f` receive one bounded high-tier rework cycle covering all four final audit findings?
  - Context: Ordinary rework is exhausted; its clean, unmerged branch otherwise blocks the thin production runner and downstream rare-event report.
  - Options: authorize the complete repair; narrow the artifact threat model and accept path-based TOCTOU risk; or leave the issue blocked.
  - Recommendation: Authorize the complete repair because descriptor-relative publication, typed destination/payload binding, OS-observed timed liveness, and durable checkpoint reconstruction jointly define the promised fail-closed boundary.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Progress and wave plan: `dev/active/b8206228-permanent-statistics/progress.json`
- Validation prerequisite: `jit issue show 02b8137c`
- Frozen protocol: `dev/simulation_results/permanent-zero-fraction/protocol.md`
- Existing exact-anchor test: `crates/gf2-algebra/tests/permanent_exact_anchors.rs`
- Production evaluator/pooling path: `crates/gf2-sim/src/permanent_campaign/schedule.rs`
- Frozen campaign: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`
- Preserved emitter: `/home/vkaskivuo/.local/state/gf2-frozen/permanent_campaign-c9b2307a`
- Validation worktree: `.agents/worktrees/agent-02b8137c` at `89c7e8cc` (clean, no worker commits)
- Importance-sampling branch: `.agents/worktrees/agent-6639435f` at `49b4d452`

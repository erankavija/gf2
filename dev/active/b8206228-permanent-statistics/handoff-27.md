# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 31

**Date:** 2026-08-30
**Session number:** 31
**Prior handoffs:** `handoff.md` through `handoff-26.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic: `b8206228` — state: backlog.
- Wave in progress: wave 10 of 13.
- Children summary: 11 direct dependencies done and 4 backlog. Pre-draw prerequisite `02b8137c` is ready; campaign arms `ed494117`, `1d0b3ec4`, and `90a61cd4` are blocked on it; `f27150a5` is blocked on all three arms.
- Active claims: no running worker and no assignment on `02b8137c`. `6639435f` remains assigned in JIT to `agent:codex-sol-importance`, but its separate branch is idle and unmerged pending the escalation below.
- Open escalations: `6639435f` awaits owner authorization for one bounded high-tier rework cycle covering its four final publication/lifecycle findings.
- Progress file: `progress.json` in the epic's artifact directory, `jit doc dir b8206228 dev/active` (reflects the above).
- Main is clean. Validation WIP is isolated on branch `worktree-agent-02b8137c` at `783d4abb` in `.agents/worktrees/agent-02b8137c`; it is not merged, compiled, tested, reviewed, or gated.
- No Cargo, nextest, validation, campaign, device, benchmark, or measurement process is running. No validation-purpose or campaign-purpose draw occurred. The frozen campaign directory contains only `manifest.json`, `checksums.sha256`, and `freeze.md`.

## What just happened

- Resumed `02b8137c`, recreated its worktree from main `b05d910d`, and dispatched one high-tier implementation worker plus one independent read-only high-tier auditor.
- The auditor mapped REQ-01 through REQ-09, calculated an approximately 85.4-million-matrix exhaustive universe and approximately 299-million selected-backend evaluations, and required bounded one-pass streaming.
- The auditor caught critical design hazards before evidence production: scheduler purpose tags were separable from the actual hard-coded campaign sampler purpose; exact-test logic risked duplication; the statistical null risked coming from preregistered counts instead of the new exhaustive result; runtime provenance was caller-forgeable; backend eligibility needed a canonical manifest authority; and batch size was being confused with worker count.
- The worker drafted an independent exhaustive visitor in `gf2-algebra`, shared semantic-purpose/evaluator/pooling seams in `gf2-sim`, canonical backend inventory/support, build/compiler and hardware observers, and a reusable validation API with durable per-anchor no-redraw journaling and focused behavioral tests.
- The owner ordered an immediate stop before the worker added the thin runner and committed strict preregistration or ran compile/test correction. The worker was interrupted and its exact filesystem state was preserved as WIP commit `783d4abb`.
- One earlier worker test scaffold leaked into main. It differed from and was superseded by the worktree version, so it was removed from main. The required leak check then passed and main is clean.
- One early focused Cargo invocation had stopped at test-target discovery because the initial patch had not landed. No test body ran. The final WIP received no compile, test, fmt, clippy, cargo-ci, configured gate, or lead review.

## What to do next

- [ ] Resume the existing `.agents/worktrees/agent-02b8137c` at `783d4abb`; do not merge or cherry-pick it first. Inspect the complete WIP diff against `b05d910d` and re-read this handoff plus `handoff-26.md`.
- [ ] Verify rather than assume the worker's reported fixes: semantic `StreamPurpose::Validation` must seed and receipt the same address; exact decisions must call `two_sided_test(...).rejects_at(0.001)` directly; sampled $p_0$ must derive from the fresh exhaustive count; production provenance must be internally observed; batch size and worker count must remain distinct; journal directory entries must be durably synchronized.
- [ ] Derive the required backend set from the frozen manifest's canonical backend union and scheduler support. A required accelerator that is unavailable fails validation; it is never silently skipped.
- [ ] Add the thin `permanent_validation` runner and committed strict preregistration for exactly the ordered ten anchors, 1,024 fresh replay matrices, 400,000 validation draws, strict 0.001 rule, no redraw, canonical authorities, output schema/path, and frozen-artifact before/after guard.
- [ ] Establish real TDD evidence and run focused Rust 1.95 release tests, fmt, focused clippy, and raw `./scripts/cargo-ci.sh`. Review semantic receipt mutations, interruption/adoption, frozen artifact isolation, and the complete literal REQ-01..REQ-09 mapping.
- [ ] Only after review, merge sequentially into clean main, run the post-merge CI contract, evaluate `cargo-ci`, `code-review`, `doc-review`, and `research-review`, and link the final immutable receipt to `02b8137c`.
- [ ] Execute the fixed validation run only after producing code and preregistration are committed, after 02:00 local under `dev/scripts/ccx1-bench-flock.sh --full-host`. Preserve the first outcome without redraw and create no campaign artifacts.
- [ ] If validation passes and `02b8137c` closes, restore and hash-check the preserved `c9b2307a…` emitter, then execute exact $(q,n)=(7,20)$ to terminal state before any other campaign-purpose cell.

## Traps — do not repeat these

- **Do not merge or cherry-pick `783d4abb` as completed work.** It is an emergency WIP preservation commit with 2,506 inserted lines, no final runner/preregistration, and no compile, test, review, or gate evidence.
- **Do not run the validation runner merely because the module compiles.** Producing code and strict preregistration must be committed first; the evidence run occurs once in the approved window.
- **Do not pass a purpose tag while constructing a `CampaignCell` sampler.** The pre-existing scheduler could receipt one tag while seeding another purpose. The semantic `StreamPurpose` or complete `MatrixAddress` must be one source for both sampling and evidence identity.
- **Do not duplicate the exact-test decision.** Use `!two_sided_test(k, N, p0).rejects_at(0.001)`; equality fails. Do not compare re-exponentiated log values, add tolerance, or use Clopper–Pearson.
- **Do not use preregistered expected counts as the sample null.** Derive $p_0$ from the newly completed exhaustive numerator and denominator; expected counts are only independent cross-check authorities.
- **Do not accept caller-authored runtime provenance.** Source closure, producing binary, build compiler/toolchain, hardware, invocation, worker count, RNG identity, and timestamps must be observed or sealed by the production path.
- **Do not confuse the bounded matrix batch size with Rayon worker count.** The interrupted draft initially passed batch size into worker-count parameters; verify the preserved replacement removed every occurrence.
- **Do not publish a journal link without syncing the containing directory.** A crash-lost start or terminal record can permit a prohibited redraw. Test stale temporary files and adoption explicitly.
- **Do not hard-code a partial backend vector.** Derive the frozen campaign backend union from the manifest and filter through one canonical scheduler support rule; a required-but-unavailable backend fails.
- **Do not materialize all exhaustive matrices or enumerate once per backend.** Stream each matrix once in bounded batches and fan identical ordinal-bound batches to every required backend and the determinant path.
- **Do not trust a serde round trip as receipt validation.** Mutate anchor coverage/order, counts, purpose, replay, backend coverage, exact value/verdict, authorities, provenance, timestamps, and overall status one at a time and require rejection.
- **Do not let worktree files leak into main.** The session caught and removed one leaked untracked test scaffold; run `check-leak-into-main.sh` again after any resumed worker.
- **Do not touch the frozen campaign directory or pinned emitter during validation implementation.** Its three-file inventory and scientific payload remain unchanged.
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
- Prior validation handoff: `dev/active/b8206228-permanent-statistics/handoff-26.md`
- Preserved validation WIP: `.agents/worktrees/agent-02b8137c`, branch `worktree-agent-02b8137c`, commit `783d4abb`
- Existing exact anchors: `crates/gf2-algebra/src/permanent/exact.rs` and `crates/gf2-algebra/tests/permanent_exact_anchors.rs`
- Shared production path: `crates/gf2-sim/src/permanent_campaign/schedule.rs`
- Frozen campaign: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`
- Preserved emitter: `/home/vkaskivuo/.local/state/gf2-frozen/permanent_campaign-c9b2307a`
- Importance-sampling branch: `.agents/worktrees/agent-6639435f` at `49b4d452`

# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 11

**Date:** 2026-08-25T17:48Z
**Session number:** 11
**Prior handoffs:** `handoff.md` (s4), `handoff-2.md` (s6), `handoff-3.md` (s7), `handoff-4.md` (s8), `handoff-5.md` (s9), `handoff-6.md` (s10)

## Current state

- Epic `6dc81018` remains `in_progress`, claimed by `agent:jit-execution-lead`.
- Wave 23.5 is blocked before `389aa4de`'s measured phase by the Block E committed-profile migration choice. Its harness phase is merged; `cargo-ci` passed, `code-review` failed on the intentionally absent measured artifact, record updates, and v2 behavioral token, and `doc-review` is pending.
- Later open direct children remain unchanged: `a83583e0` and `5bdc9552` are ready; `eaae1b56`, `dbd8787d`, and `06ba0418` wait on their recorded dependencies.
- No epic file, JIT issue state, gate, benchmark, or Cargo command changed this session. This handoff and `progress.json` are the only session changes.
- Claude independently leads OSD epic `b7157be6`. Coordination uses `/home/vkaskivuo/Projects/forum-poc` as `agent:codex` ↔ `agent:claude`; its paths and issues do not overlap this epic. Shared Cargo, Lean, and benchmark work must still be serialized.

## What just happened

- Recovered one stale JIT claims lock, re-read repository configuration, all prior handoffs, the current progress file, remaining issue contracts, gates, linked artifacts, and the execution-lead review/escalation protocols.
- A high-tier read-only audit of `389aa4de` confirmed the current source still emits `tuning-calibration-v1`. The harness declares any dirty-source emission non-committable, so the token cannot be edited only in the working tree before measurement.
- A clean commit that changes the parser to v2-only immediately rejects `crates/gf2-core/data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json`. Retagging that old artifact as v2 is also invalid: its binary SHA identifies the v1 producer, so relabelling violates `@/inv/behavioral-evidence-validity` and `@/inv/runtime-observed-provenance`.
- The safest replace path is a named temporary compatibility boundary on an issue branch: commit a v2-emitting loader that temporarily accepts v1 and v2 while keeping the old v1 artifact byte-identical; run the benchmark from that clean revision; then replace the v1 anchor with the measured v2 profile, update all readers/records, remove v1 acceptance, gate the complete series, and merge it to main. This requires owner approval because it is a shared profile-format migration and clarifies DEC-B17's “same change” as one reviewed commit series.
- Claude completed OSD issue `c97b2961`, offered an exclusive host window, and held its next wave. The window was released without running anything once the migration blocker was confirmed. Request a fresh 45-minute window after the owner decides.

## What to do next

- [ ] Obtain the owner decision in **Open questions**. Record it in `progress.json` and clear `open_escalations`.
- [ ] Ask Claude for a fresh 45-minute exclusive host window before any benchmark build or timed run.
- [ ] For the recommended replace path, prepare the temporary migration in a fresh worktree/branch anchored to current `main`; do not reuse `.agents/worktrees/agent-389aa4de`, whose branch now carries unrelated unmerged work.
- [ ] Build and run the calibration with Rust 1.95, features `tuning-profile,simd`, five executions, five repetitions, 250 ms windows, an absent `/tmp` output, and `dev/scripts/ccx1-bench-flock.sh`. Record separate host-idle evidence because the lock wrapper serializes but does not admit only an idle host.
- [ ] Validate `source_dirty:false`, v2 token, route/digest/window checks, all nine Karatsuba grid rows, explicit selection/fallbacks, omission complement, successful reload, receipt path, output digest, and runtime provenance before copying the artifact byte-for-byte.
- [ ] Apply Blocks A–D and the Block E reader/test/doc inventory from `dev/active/389aa4de/receipt-notes.md`; also supersede `dev/active/220cab0b/design.md`'s v1 example and mark `receipt-notes.md` applied. If `simd_min_words` moves, produce the required DEC-G re-pin evidence.
- [ ] Re-run all three `389aa4de` gates, perform the six-tier lead review including cumulative findings, then close it before proceeding to later waves.

## Traps — do not repeat these

- **Do NOT run the benchmark from the current v1-emitting source and retag its JSON as v2.** The output token is the producer's behavioral identity; changing it after emission contradicts the recorded binary SHA.
- **Do NOT edit the token only in the working tree before measuring.** The harness records `source_dirty:true`, and its own contract says that output is not committable.
- **Do NOT rewrite the old v1 profile's token from its recorded numbers.** The old binary emitted v1; relabelling it as v2 is not runtime-observed provenance.
- **Do NOT assume `ccx1-bench-flock.sh` proves host idleness.** It provides the mutex, affinity, and best-effort niceness only. Record a quiet-host preflight separately and coordinate Claude's build waves through the forum.
- **Do NOT reuse or reclaim `.agents/worktrees/agent-389aa4de`.** It is clean but its branch contains unrelated unmerged commits for another issue; create a distinct fresh worktree after the owner decision.
- All traps from `handoff-6.md` and its chain remain in force, especially: benchmark-mode never overlaps other builds; main gate chains own the Cargo lock before worker CI; no merge/commit during a gate evaluation; closed records are amended append-only except explicit state cells; and worker claims/reports never replace artifact inspection.

## Open questions needing invoker input

- Question: which committed calibrated-profile migration shape should `389aa4de` use?
  - Context: the benchmark must run from clean v2-emitting source, while the only committed profile is a v1 behavioral artifact that a v2-only loader rejects. An atomic token-bump-plus-measurement commit is impossible because the measurement can occur only after the binary exists.
  - Options: (A) **replace** the v1 anchor through a temporary, named dual-token migration commit, then commit the measured v2 replacement and remove v1 acceptance before merging the reviewed series; (B) **add beside** the v1 anchor, which requires a lasting versioned compatibility boundary while the v1 artifact remains and leaves two calibrated authorities.
  - Recommendation: (A). It preserves truthful provenance, converges to one canonical committed profile, removes the old `source_dirty:true` anchor, and confines compatibility to a reviewed migration boundary with an immediate removal condition.

## Reference artefacts

- Epic and progress: `jit issue show 6dc81018`; `dev/active/6dc81018-field-capability-dispatch/progress.json`
- Current issue and checklist: `jit issue show 389aa4de`; `dev/active/389aa4de/receipt-notes.md`
- Governing designs: `dev/active/220cab0b/design.md`; `dev/active/7d824b2f/design.md`; `dev/active/7d7c647c/design.md`
- Current profile and receipt: `crates/gf2-core/data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json`; `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`
- Host coordination: `/home/vkaskivuo/Projects/forum-poc/README.md`; peer mailbox `agent:claude`

## Resolution — 2026-08-25

The owner chose option A and explicitly grounded it in
`@/invariant/canonical-cutover` and `@/invariant/convention-convergence`.
Decision DEC-V replaces the v1 anchor through the named temporary dual-token
migration boundary described above. Its tracked removal condition is
pre-merge: v1 acceptance and the superseded v1 profile are both absent from
the reviewed final tree, leaving one v2 profile and one parser contract.

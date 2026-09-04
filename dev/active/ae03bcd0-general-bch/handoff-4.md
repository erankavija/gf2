# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 5

**Date:** 2026-09-01T23:55+03:00
**Session number:** 5 (session 4 died at a rate limit mid-wave-8 and left NO handoff; this session reconstructed its state from git log + gate records)
**Prior handoffs:** handoff.md, handoff-2.md, handoff-3.md — their Traps sections remain in force in full.

## Current state

- Epic `ae03bcd0` — state: backlog (container blocks until subtree terminal); claimed by agent:jit-execution-lead.
- **Waves 1–9 CLOSED.** `current_wave` = 10 in progress.json; wave 10 NOT dispatched (owner directed session end at wave close).
- Children: 40 done, 0 in_progress, rest backlog/ready per progress.json waves 10–15. No rejections.
- Active claims: epic by agent:jit-execution-lead; the four wave-9 issues were re-claimed by the lead for gate runs and are done. No worker claims outstanding.
- Open escalations: none.
- Progress file: `progress.json` here (current; includes session_notes_s5 and rulings R-28..R-30).

## What just happened

- Reconstructed session-4 state: 18a160f5 + 64fd3afd done; 6d67e57e/8f68699b/c3cc5226 had code-review R1 FAILs recorded, 6d67e57e's rework was orphaned uncommitted in worktree agent-6d67e57e-r2.
- `6d67e57e`: preserved orphan WIP (37074ddf), Luna finished it (canonical `ParityCheckMatrixAccess for Extended<C>` + shared eBCH fixture in test_support + sim_runner swap, ruling R-28); merged fb095d61; both gates passed; done.
- `8f68699b`: R1 rework (Opus) — allocation-free `encode_into` via per-thread TypeId-keyed scratch (in-repo precedent gf2m/wide.rs), merged 67d35782; code-review R2 surfaced NEW pre-existing finding (encode_partitions fewer chunks than workspaces); R2 rework — balanced partitions via recursive halving + rayon::join, merged 0c4830e0; code-review R3 PASS; done.
- `c3cc5226`: R1 rework (Opus) — restore-on-error + recoverability-split CPU fallback, `HipError::is_recoverable` added at the error owner (gf2-kernels-hip), receipt repinned; merged 63952bf2; code-review R2 surfaced NEW finding (exp/log-table `.expect` panics on valid canonical fields without u16 tables); R2 rework — `device_syndromes_supported` predicate + CPU fallback on both APIs, legacy `BchDecoder` deliberately left panicking (type-wide table precondition, untestable branch — reverted rather than shipped); receipt repinned 489e85ef, doc-linked; all three gates passed; done. Wave 8 closed.
- Wave 9 dispatched (4 worktrees at 1ae08726): `5cbdec87` (Luna) single legacy HIP-test consumer migrated + device self-gate, all gates first-pass, done. `571aaf80` (Opus) NonPrimitiveConsecutive variant + shared normalize_consecutive tail, gates first-pass, README BCH-row staleness closed lead-direct, done. `41090b8d` (Opus) proofs/Gf2Core/Proofs/RelativeExtension.lean (60 decls, 0 sorry, axiom audit clean; only inherited Aeneas Formatter axiom on extracted carriers), lake-build + doc-review + code-review first-pass, done. `177bdc85` (Opus) EncodeFamily dispatch seam + table-remainder family + CodingTuning section mirroring gf2-algebra, cargo-ci first-pass (19 steps incl. its 3 new tuning-coding steps, ruling R-29), code-review R2 after rework R1 (workspace-time family table prep), done. Wave 9 closed.
- Rulings: R-28 (sim_runner swap in 6d67e57e), R-29 (cargo-ci.sh tuning-coding steps), R-30 (workload-selection §9 amendment: T2S/T2N differential at mother length 16383/65535 until 97410c80 migrates DVB-T2; 4e732b56 doc link refreshed).
- All worker worktrees reclaimed, branches deleted; leak checks clean throughout. agent-4e732b56 KEPT (built baselines).

## What to do next

- [ ] Dispatch wave 10 per progress.json: `2b6968d3` avx2-batch-kernels (hard → Opus; registers via the EncodeFamily seam — one enum variant + `family_available` arm + reduction; survey note: check generated code first for the interleaved-decay question; § 7 fixes the `bitslice-interleaved` name/contract), `1c9a0f9f` bch-convenience-ctors (easy → Luna; design assigns every delegating convenience on bch/spec.rs to it; R-25 context in progress.json), `bd0edfa2` genmatrix-perf (hard → Opus; survey §8.1: REPLACE the basis-vector algorithm — M4RI repository-order fresh-alloc route is the measured target; built baselines live in worktree agent-4e732b56), `32f53280` lean-quotient-reduction (hard → Opus; R-27: REQUIRED tests canonical_index_decodes_to_its_prime_coordinates (GF(16),GF(125),GF(81)/GF(9)) + reduction_is_invariant_under_multiples_of_the_modulus + tower_coordinates_vary_the_base_coordinate_fastest; module + one import line only).
- [ ] Standing: `88ca7d2f` (pin pre-cutover baseline receipt, wave 11, CCX1 host) must land before cutover-benches (591a1c5e, wave 12) merges.
- [ ] Bake into perf-wave prompts (fd9d5416/d1b4f85e): tuning-profile-compose needs a coding-owner mode (pitfall from 177bdc85); re-establish absolutes on the kept governor; benchmark receipts only from committed revisions.
- [ ] Epic-close leftovers in surfaced_pitfalls: verify-lean.sh:36-40 stale override count (fold into any verify-lean-touching task or lead-direct at close).
- [ ] Wave-10 conflict note: 2b6968d3, 1c9a0f9f, bd0edfa2 all touch gf2-coding; 1c9a0f9f (spec.rs) vs 2b6968d3 (encode.rs + kernels-simd) vs bd0edfa2 (matrix.rs) are file-disjoint on their cores but may all touch bch/mod.rs re-exports — expect trivial merge friction, serialize merges as usual.

## Traps — do not repeat these

- **A code-review round N pass set does not bound round N+1.** Both 8f68699b and c3cc5226 drew NEW blocking findings on R2 in code the rework never touched (the reviewer re-reads the issue's whole attribution). Budget for it: a "one finding, one fix" plan is optimistic; run the rework, expect the reviewer to probe adjacent contract surfaces next round.
- **A new algorithm family must satisfy ALL previously-completed contracts on its path, not just its own issue's.** 177bdc85's table family lazily built tables inside the first workspace-batch call and was failed against 8f68699b's REQ-01 (allocation-free), an issue that was already done. Before dispatching work that plugs into a completed surface, name the completed contracts it must uphold in the prompt.
- **Do NOT ship an untestable fallback branch to satisfy a review pattern.** c3cc5226's worker implemented a fallback for the legacy decoder, found via mutation testing that no test could make it fire (type-wide precondition), and REVERTED it, documenting why. The reviewer accepted this. Honest revert + rationale beats dead defensive code.
- **The stale-pooled-artifact trap (handoff-3 era) is STILL LIVE:** gf2pow32_constant_drift failed again in a fresh worktree from a pool-seeded binary baked with another worktree's path (177bdc85 session). Purge gf2pow32_* from the pool after each harvest, or expect one spurious failure per wave until the cargo-budget.sh upstream fix.
- **codex sandbox blocks the GPU device** (hipGetDeviceCount error 100) even on this HIP-capable host — a Luna worker's device-test failures are sandbox artifacts, not regressions; the lead's gf2-kernels-hip-ci gate outside the sandbox is the real evidence. Also codex sandbox may need `CARGO_CI_NO_LOCK=1 CARGO_CI_NO_SCCACHE=1` for the budget wrapper's lock paths.
- **Worker reports truncate ~3KB in SendMessage transport** — instruct workers to lead with essentials; plan one extra round-trip per report for the tail. (Carried from handoff-3, confirmed 4× this session.)
- **jit doc links pin content hashes:** amending ANY linked doc (even another done issue's, e.g. workload-selection.md) requires a `jit doc remove` + `add` refresh, or the link dangles stale. Done for 4e732b56 and the c3cc5226 receipt this session.
- All traps in handoff.md, handoff-2.md, handoff-3.md remain in force (codex startup-deadlock probing, gate-evaluate mutation-retry freeze, `cmd | tail` exit-code masking, claim-release-reclaim, .jit porcelain checks, never reclaim agent-4e732b56, no-delegation clause in every hard-task prompt, done-transitions respect the DAG).

## Open questions needing invoker input

None. Wave 10 dispatch is fully specified above.

## Reference artefacts

- Epic: `jit issue show ae03bcd0`; progress.json, plan.md, breakdown.json, investigation.md, bch-api-design.md, extension-design.md (this directory).
- Proof sketch (spec for remaining lean waves): dev/active/64fd3afd/proof-sketch.md; R-27 rulings in progress.json lead_decisions.
- Survey contract (amended §9): dev/active/4e732b56/workload-selection.md; findings.md; baselines in KEPT worktree agent-4e732b56; receipts dev/bench_results/4e732b56/.
- HIP receipt: dev/bench_results/c3cc5226/hip-outcome-identity-receipt.md (pinned 489e85ef, doc-linked).
- New shared surfaces wave-10 workers consume: EncodeFamily seam + CodingTuning (crates/gf2-coding/src/bch/encode.rs, tuning.rs), NonPrimitiveConsecutive (bch/spec.rs), ParityCheckMatrixAccess for Extended (transform/mod.rs), RelativeExtension.lean (proofs/Gf2Core/Proofs/).

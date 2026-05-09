# Handoff — Fast matrix permanents over F_3 / F_5 / F_7 via packed bipedal arithmetic (ae82bd73) — session 1, part 2

**Date:** 2026-05-09 (continuation after the part-1 mid-session handoff)
**Session number:** 1 (resumed; this supersedes ae82bd73-handoff-1.md)
**Prior handoffs:** ae82bd73-handoff-1.md (early-session state when only W0-0a sketches had landed)

## Current state

- Epic: `ae82bd73` — state: `backlog` (still has 41 unresolved deps, but every W0 issue is now `done`)
- Wave in progress: **W0 fully closed; W1 ready to dispatch**.
- Children summary: 11 done, 2 ready (T1 + T16), 39 backlog, 0 rejected.
- Active claims: none — all in_progress claims of session-1 closed at end of session.
- Open escalations: none.
- Progress file: `dev/active/ae82bd73-progress.json` (reflects the above).

## What just happened (since handoff-1)

- **D1c (4fced99b)** dispatched + rework cycle 1 (verification-approach: full sweep vs CI subset). Round 2 PASS. Closed.
- **R4 (c7542983)** dispatched + rework cycle 1 (bench input methodology bug + 3rd run claim). Round 2 PASS. Decision: **generic BatchedBipedalLike framework wins** (all 27 cell-runs in [0.83, 1.20] tie-break band; ratio range 0.931-1.091). Closed.
- **D1b (9fe275d3)** went through 4 rework cycles before passing. Each round surfaced a different real finding (LANES contradiction → missing approval → criterion-2 wording → missing tests + doc-examples → missing # Complexity). Final pass after a comprehensive doc-standards self-audit covering description + Arguments + Examples + Panics + Complexity on every public item.
- **41 W1-W7 issues created and dep-edges wired** by a parallel sub-agent dispatch. JIT validate passes; transitive-reduction validator collapsed redundant edges.
- **8e4e19a0 (perm-vs-det uniformity)** rewired to additionally depend on T9, T18, T20 (the actual `permanent_*` implementations). Previously mis-wired — would have dispatched before W2-W4 work landed.
- **CLAUDE.md** §Architecture and §Key design invariants amended to permit `dev/research/<crate>/` stubs to contain `unsafe` if each `pub unsafe fn` carries a `// SAFETY:` top-of-function comment (D4 user policy decision applied).

## What to do next

**Immediate (next session start):**

- [ ] Claim and dispatch **T1 (e6b1216a — gf2-algebra crate skeleton)**. This creates `crates/gf2-algebra/` with the `[features]` block per D1c, the `Cargo.toml` workspace member entry, `#![deny(unsafe_code)]`, and the empty module skeleton (`packed/`, `permanent/`, `gray.rs`). Verify with the W1-T1 acceptance gate: full 64-cell `cargo check --features <combo>` sweep per D1c §6.1 (~30-60 min wall-clock).
- [ ] In parallel with T1: claim and dispatch **T16 (b17bec62 — BatchedBipedalLike framework)**. Implements the generic SIMD framework chosen in R4. Lives in `crates/gf2-kernels-simd/`. File-disjoint with T1 so safe to parallelize.
- [ ] After T1 closes, dispatch **T2 (1aa0cb99) + T3 (46330802) + T6 (94870b84)** in parallel (W1 sub-wave 1b). T2 = scalar PackedField impl. T3 = Bipedal3 element with paper formulas + proptest tests. T6 = gray_code_iter shared subset enumerator.
- [ ] After T3 closes, dispatch **T4 (053e4016 — Bipedal3Vec)**.
- [ ] After T4 closes, dispatch **T5 (ef7d0633 — Bipedal3Matrix column-major)**. Note: COLUMN-major per the R3/D1a SSOT fix in session 1.
- [ ] After T6 closes, W2 unblocks: dispatch **T7 (93e5a5e8) + T9 (b0857ae9)** in parallel. T7 = generic permanent_ryser. T9 = permanent_bipedal3 single-word fast path. (T8 reference port also unblocks once T7 closes.)
- [ ] Continue down the wave plan in `dev/plans/gf2_algebra_permanent.md` §13.

**Process improvements (lessons embedded into next-session dispatch):**

- [ ] **Pre-flight every dispatch with the CLAUDE.md doc-standards checklist** embedded in the agent prompt: every public item must carry description + # Arguments (if non-self args) + # Examples (runnable doc-test) + # Panics (if applicable) + # Complexity (for non-trivial ops). D1b lost 4 review cycles to incremental doc-standards findings; pre-flight prevents this.
- [ ] **Pre-flight every test-touching dispatch with the CLAUDE.md testing checklist**: TDD, proptest for mathematical invariants, word-boundary cases at {0, 1, 63, 64, 65, 127, 128, 129}, doc-test examples on every public API, test naming `test_<operation>_<scenario>`.
- [ ] **For every dispatch creating a `dev/research/<crate>/`**: enforce a `.gitignore` with `target/` + `Cargo.lock` BEFORE the first commit, per memory feedback `feedback_dev_research_target_gitignore`.
- [ ] **For every issue with a `[hard]` performance criterion**: include a measured baseline in the dispatch prompt; the reviewer rejects "based on intuition" claims.
- [ ] **For every bench/microbench dispatch**: the doc claims about run count, methodology, input distribution MUST match the bench code exactly. Self-audit before submitting; D1b/D1c/R4 each lost a cycle to mismatch findings.
- [ ] **For every issue with a sketch/approval criterion**: pre-amend criterion to use the project's `## Approval` description-edit convention (precedent: D2/D3/D1b session 1) BEFORE dispatching the worker. Saves a rework cycle.

## Traps — do not repeat these

Carrying forward from handoff-1 + adding session-1's lessons. **All traps remain in force unless explicitly resolved.**

- **DO NOT use a JIT "comment" primitive — there isn't one.** Sketch / trait-surface / decision-doc approval criteria that say "recorded as a comment / approval note" must be satisfied via a `## Approval` section in the issue description (project-wide convention; see D2/D3/D1b for precedent). Reviewer LLM is stochastic on this — pre-amend the criterion in the issue description before dispatching the worker.

- **DO NOT cite `gf2-kernels-simd → gf2-core` as a forward edge.** The actual workspace dependency is `gf2-core → gf2-kernels-simd` (cfg-gated `simd`, default-on). Verify at `crates/gf2-core/Cargo.toml:15-21`.

- **DO NOT use `(k >> flip) & 1` for Gray-code add/sub.** With `flip = trailing_zeros(k)`, bit `flip` of `k` is always 1. Use `g_k = k ^ (k >> 1); ((g_k >> flip) & 1) == 1`. Hand-verified for k=1..4 in `dev/plans/r3_multi_word_streaming.md`.

- **DO NOT cite epic doc subsections "§12.1" / "§12.2" / "§12.3"** — section 12 has named subsections V1, V2, V3.

- **DO NOT assume `dev/research/` stubs are exempt from CLAUDE.md unsafe-isolation rule WITHOUT the explicit carve-out.** As of 2026-05-09, the rule is amended (CLAUDE.md §Architecture and §Key design invariants 3) to permit dev/research/<crate>/ stubs that exercise an unsafe surface, provided each `pub unsafe fn` has a top-of-function `// SAFETY:` comment.

- **DO NOT treat trait surface as approved without the `## Approval` section literally present in the issue description.** The reviewer LLM scans for this string; missing it is an automatic FAIL.

- **DO NOT submit a deliverable until it passes a self-audit against ALL of CLAUDE.md's documentation standards (description + Arguments + Examples + Panics + Complexity) AND testing standards (proptest for invariants, word-boundary at {0, 1, 63, 64, 65}, doc-tests on public APIs).** The reviewer LLM cycles through different standards in different rounds; pre-audit prevents the multi-cycle pattern.

- **DO NOT recommend a "subset" of a 2^N feature matrix as the verification approach when the criterion says "running cargo check over the matrix".** State the full sweep as the verification approach (W1-T1 acceptance, ~30-60 min one-time); the subset is a downstream CI cost-control measure, NOT the verification approach. (D1c precedent.)

- **DO NOT dispatch parallel agents on issues that share working-tree files.** Reviewer flags it as "change not scoped to a single issue". For W0 sub-wave 0a we accepted the noise (different docs, different .jit/<id>.json); for W1+ where multiple agents may touch shared crate sources, use worktree isolation per `references/worktree-dispatch-protocol.md` if the project-lead skill ships one.

- **DO NOT commit `dev/research/<crate>/target/` build artefacts.** EVERY new `dev/research/<crate>/` MUST have a `.gitignore` with `target/` + `Cargo.lock` in its first commit. Captured in `feedback_dev_research_target_gitignore` memory.

- **DO NOT forget to claim + transition to in_progress BEFORE Agent dispatch.** Per memory `feedback_jit_claim_before_dispatch`. The lead pre-flight is: `jit issue claim <id> agent:claude` + `jit issue update <id> --state in_progress`, THEN dispatch.

## Open questions needing user input

None as of session 1 close. All four session-1 escalations resolved.

## Reference artefacts

- Epic: `jit issue show ae82bd73`
- Epic design doc: `dev/plans/gf2_algebra_permanent.md`
- W0 design/research artefacts (all attached to their issues):
  - `dev/plans/d1a_gf2_algebra_boundary.md` — D1a
  - `dev/plans/d1b_packed_field_api.md` + `dev/research/packed_field_stub/` — D1b
  - `dev/plans/d1c_feature_matrix.md` — D1c
  - `dev/plans/d2_lean_bipedal3_sketch.md` — D2
  - `dev/plans/d3_lean_ryser_sketch.md` — D3
  - `dev/plans/d4_intrinsic_feasibility.md` + `dev/research/intrinsic_feasibility_stub/` — D4
  - `dev/plans/r3_multi_word_streaming.md` — R3
  - `dev/plans/r4_simd_batching_decision.md` + `dev/research/simd_batching_bench/` — R4
- Progress: `dev/active/ae82bd73-progress.json` (current; supersedes session-1 mid-state)
- Prior handoff: `dev/active/ae82bd73-handoff-1.md`
- Project rule amendments (2026-05-09): `CLAUDE.md` §Architecture and §Key design invariants 3 (unsafe-isolation carve-out for dev/research/ stubs).
- Memory additions (2026-05-09): `feedback_dev_research_target_gitignore`.
- External references unchanged from handoff-1.

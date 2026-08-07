# Handoff — Fast matrix permanents over F_3 / F_5 / F_7 via packed bipedal arithmetic (ae82bd73) — session 1

**Date:** 2026-05-09
**Session number:** 1
**Prior handoffs:** None.

## Current state

- Epic: `ae82bd73` — state: `backlog` (cannot transition to `in_progress` until all child deps clear)
- Wave in progress: W0 (Decisions and sketches)
- Children summary: 2 done (R1 F_5, R2 F_7 — completed in prior session), 5 in_progress with rework cycle pending or escalation pending, 4 still backlog/ready
- Active claims (all `agent:claude-opus-4-7`):
  - `6e20133d` (D1a)
  - `a0c0a45f` (D2)
  - `4aaa6e4d` (D3)
  - `4c534d31` (D4)
  - `60c30e2d` (R3)
- Open escalations: 4 (see "Escalations" section below). All blocking W0 closure.
- Progress file: `dev/active/ae82bd73-progress.json` (reflects the above)

## What just happened

- Read epic + design doc + 11 children. Identified W0 sub-DAG: 5 issues independent (D1a, D2, D3, D4, R3) + 3 chained (D1b → D1c, R4) + 8e4e19a0 mis-wired (depends on W2-W4 deliverables, not W0).
- Claimed 5 sub-wave 0a issues (D1a, D2, D3, D4, R3) under `agent:claude-opus-4-7`. Transitioned to `in_progress`.
- Dispatched 5 parallel general-purpose agents to produce decision/sketch docs.
- All 5 docs landed (in `dev/plans/d{1a,2,3,4}_*.md` and `dev/plans/r3_multi_word_streaming.md`); D4 also created stub crate `dev/research/intrinsic_feasibility_stub/`.
- Ran cargo-ci on all 5 — all passed (4 min for first warm-up, seconds each for subsequent).
- Ran code-review on all 5 — all FAILED (round 1).
- Dispatched 4 reworks (D1a, D2, D3, R3) in parallel based on round-1 findings. D4 held pending unsafe-isolation policy decision.
- All 4 reworks completed; surgical Edit to fix one extra finding the D3 worker spotted in D2 line 59 (broken `epic doc §12.1` reference, mirror of the D3 §12.2 defect).
- Re-ran code-review on D1a, D2, D3, R3 — all FAILED (round 2). New findings:
  - **D1a**: criterion 2 strict reading conflicts with feature-gated kernel edges. Criterion-amendment escalation needed.
  - **D2**: user-approval criterion still unmet (deferred to lead per design); reviewer also flagged "path target doesn't exist locally" (gf2-algebra crate lands in W1-T1, not W0); reviewer wants explicit JIT dependency edge from a0c0a45f to 6e20133d.
  - **D3**: user-approval criterion is the only remaining blocker.
  - **R3**: Gray-code rule fixed correctly, but `gray_code_iter` interface is now described two different ways in §6 (yields k=0,1,...) vs §8 (consumed as `(k, flip)` from `.enumerate().skip(1)`). Fixable in cycle 2 rework.
- D4: held throughout. Reviewer's round-1 finding stands (unsafe in dev/research/ violates project rule per literal reading of CLAUDE.md).

## What to do next

In priority order:

- [ ] **Resolve the 4 escalations with the user** (see "Escalations" section). All four block W0 closure.
- [ ] After user decides on D1a criterion amendment: edit issue 6e20133d description (success criteria block) per the chosen amendment language. Re-run code-review. Then transition to done if pass.
- [ ] After user approves D2 sketch: post the approval as a JIT comment on a0c0a45f (the success criterion mentions "comment / approval note"). Re-run code-review. Transition to done if pass.
- [ ] Same for D3 sketch on 4aaa6e4d.
- [ ] After user decides on D4 unsafe-isolation policy: apply chosen fix (move stub, amend CLAUDE.md, or keep as-is with attestation), re-run code-review. Transition to done if pass.
- [ ] Dispatch R3 rework cycle 2 to fix `gray_code_iter` interface contradiction in §6 vs §8 of `dev/plans/r3_multi_word_streaming.md`. Re-run code-review. Transition to done if pass. (Cycle 2 is rework attempt 2 of 2 max; further failure escalates.)
- [ ] **Add JIT dependency edge from a0c0a45f → 6e20133d** so D2 cannot dispatch before D1a is settled (reviewer's specific request; the lead missed this in initial wiring).
- [ ] **Rewire 8e4e19a0** (perm-vs-det uniformity): currently a direct dep of the epic, but it requires implemented packed permanents (W2-W4 deliverables). Remove the epic-level dep edge, add deps on T9 (permanent_bipedal3 single-word), T18 (permanent_bipedal5), T20 (permanent_bipedal7) once those are created. This is wiring work on the JIT graph, no doc edits needed.
- [ ] After W0 closes (D1a → D1b unblocks): claim D1b (9fe275d3 — PackedField trait surface) and dispatch. D1b → D1c (4fced99b feature-gate matrix) and R4 (c7542983 SIMD batching strategy) become ready.
- [ ] After W0 fully closes: plan W1 (~6 tasks: crate skeleton T1, PackedField scalar impl T2, Bipedal3 element T3, Bipedal3Vec T4, Bipedal3Matrix T5, gray_code_iter T6). Create issues, wire DAG, dispatch.

## Escalations

These four require the user's input before W0 can close.

### 1. D1a (6e20133d) — criterion 2 amendment

**Current criterion 2:** "[hard] No circular crate dependencies are proposed; `gf2-algebra → gf2-core` is the only forward edge required."

**Reviewer's reading:** strict — gf2-algebra must have NO other forward edges.

**Architectural reality:** gf2-algebra needs:
- `gf2-algebra → gf2-kernels-simd` (cfg `feature = "simd"`, default-on) for AVX2/AVX-512 bipedal kernels.
- `gf2-algebra → gf2-kernels-hip` (cfg `feature = "hip"`, default-off) for GPU permanent kernels.

These are inherent to the epic's scope (epic doc §5 architecture, §10 SIMD batching, §11 GPU strategy). Removing them is impossible.

**Options:**
- **(A) Amend criterion 2** to: "[hard] No circular crate dependencies are proposed; `gf2-algebra → gf2-core` is the only mandatory non-feature-gated forward edge into other workspace crates. Feature-gated edges to `gf2-kernels-simd` (default-on `simd`) and `gf2-kernels-hip` (default-off `hip`) are permitted." (Lead recommendation.)
- **(B)** Keep criterion 2 as-is; reject the issue (D1a is unable to satisfy it as written). User would then decide whether to redesign the architecture or accept that D1a's product is the right architecture and the criterion was mis-written.

### 2. D2 (a0c0a45f) — Lean F_3 bipedal sketch user approval

**Sketch:** `dev/plans/d2_lean_bipedal3_sketch.md`

Highlights:
- 20 lemmas: 6 decoder ψ + 4 lane-level (decide on ZMod 3 truth tables) + 4 word-level (BitVec.getLsbD lifting) + 4 packed-correctness + 1 lifting helper + 1 headline corollary.
- All four operations covered: add, sub, mul, div (paper Theorem 2.1 formulas, verbatim).
- No new FunsExternal axioms required (purely bitwise ops on u64).
- Extraction target: `crates/gf2-algebra/src/packed/bipedal3.rs::Bipedal3::{add, sub, mul, div}` (per D1a §2 boundary).
- Estimated proof file size: ~150 lines.

**Reviewer also flagged a temporal issue:** the sketch references the `gf2-algebra` crate which doesn't exist in the workspace yet (it lands in W1-T1). Reviewer wants the crate to exist before D2 closes. This is structural — the sketches are designed to land BEFORE implementation. Options:
- **(A)** User approves now; document path is treated as a contract for W1-T1 to honour. (Recommended — matches CLAUDE.md verification-work intent.)
- **(B)** Defer D2 closure to after W1-T1 lands the crate.

**Decision needed:** Approve, request changes, or amend criterion 5.

### 3. D3 (4aaa6e4d) — Lean Ryser bounded n ≤ 63 sketch user approval

**Sketch:** `dev/plans/d3_lean_ryser_sketch.md`

Highlights:
- 9 named lemmas: 3 Gray-code (binary-reflected register, flip-bit bound, subset bijection) + 4 Ryser-formula (column-sum invariant, inner product, outer alternating sum, Mathlib Ryser identity) + 2 connecting (top-level chain).
- L7 (`ryser_eq_permanent_zmod`) is the most algebra-heavy: pure-Mathlib identity from inclusion-exclusion, estimated 30-80 Lean lines. Candidate Mathlib upstream after V2 stabilises.
- L1-L3 Gray-code requires a project-local `Gf2Algebra.Proofs.Gray` namespace (Mathlib has no formalisation; verified by grep against pinned mathlib4 v4.28.0-rc1).
- Bounded n ≤ 63 rationale: single-word Gray register, finite-arity Ryser sum, production code path bound, risk-register alignment.
- Extraction target: monomorphised `permanent_ryser_fp3 : &Bipedal3Matrix → Fp<3>` (NOT generic `permanent_ryser<F>`).

**Decision needed:** Approve sketch as-is; once recorded as JIT comment, criterion 6 is satisfied.

### 4. D4 (4c534d31) — unsafe-isolation policy

**Stub crate:** `dev/research/intrinsic_feasibility_stub/` — standalone (NOT a workspace member), exercises 5 AVX2 + 5 AVX-512F intrinsics in `#[target_feature]`-attributed `unsafe fn`s.

**Reviewer's strict reading of CLAUDE.md** §"Architecture": "Unsafe code lives exclusively in these two kernel crates" (gf2-kernels-simd, gf2-kernels-hip). The stub violates this.

**Practical reality:** dev/research/ stubs are sandbox-experiments — `rns_prototype/`, `f5_packing/`, `f7_packing/`, `f3_bipedal/` exist as standalone non-workspace prototypes. The unsafe-isolation rule was clearly written for production crates.

**Options:**
- **(A) Amend CLAUDE.md** to add "dev/research/ stubs may contain unsafe code if necessary to exercise the surface they prototype, with a SAFETY comment explaining why the unsafe is needed and what it isolates." (Lead recommendation — matches existing project convention.)
- **(B) Move the stub** into `crates/gf2-kernels-simd/research/intrinsic_feasibility_stub/` as an excluded sub-Cargo. Awkward but satisfies the literal rule.
- **(C)** Accept the failure; treat D4's deliverable as the stub-less plan in the markdown only (delete the stub crate). Loses the empirical `cargo check` verification but stays inside the rule.

**Decision needed:** Pick a path so D4 can close.

## Traps — do not repeat these

- **Do NOT dispatch sketch-approval criteria as "deferred to lead" without first auditing.** Per project memory feedback "Pre-dispatch criterion audit" and "Hard criteria self-satisfied, not deferred", a `[hard]` criterion that requires user approval is not deferrable by the worker — the lead must obtain the approval before transitioning. The reviewer correctly flags it as failing every cycle. Lead must escalate FIRST or amend the criterion FIRST. This trap cost rework cycle 1 on D2 + D3.

- **Do NOT cite `gf2-kernels-simd → gf2-core` as a forward edge.** The actual workspace dependency is `gf2-core → gf2-kernels-simd` (cfg-gated `simd` feature, default-on); `gf2-kernels-simd` only has a `[dev-dependencies]` back-edge to `gf2-core`. Verified at `crates/gf2-core/Cargo.toml:15-21` and `crates/gf2-kernels-simd/Cargo.toml:21-25`. The original D1a doc had this backwards; was fixed in rework round 1 but is a recurring confusion class.

- **Do NOT use `(k >> flip) & 1` for Gray-code add/sub.** With `flip = trailing_zeros(k)`, bit `flip` of `k` is always 1 — so the test always says ADD, never SUB. Use `g_k = k ^ (k >> 1); ((g_k >> flip) & 1) == 1` instead. Hand-verified table for k=1..4 in `dev/plans/r3_multi_word_streaming.md` rework report. R3 round-1 had this bug.

- **Do NOT cite epic doc subsections "§12.1", "§12.2", "§12.3"** — section 12 (Lean verification) has named subsections V1, V2, V3, NOT numbered subsections. The correct citation form is `§12 V1`, `§12 V2`, `§12 V3`. D2 and D3 round-1 docs both had this bug; both fixed in rework round 1.

- **Do NOT assume `dev/research/` stubs are exempt from CLAUDE.md unsafe-isolation rule.** Reviewer reads it strictly. Even though existing convention has `dev/research/` stubs (rns_prototype, f5_packing, etc.), the rule does not have an explicit carve-out. Until the user amends CLAUDE.md, expect strict enforcement.

- **Do NOT have D2 and D3 sketches close before crate `gf2-algebra` exists.** The reviewer flagged this on D2 — sketch references a non-existent path. The crate lands in W1-T1, after the sketches. The natural fix is to amend the closure logic (sketch landing + user approval = done; crate existence not required), but the reviewer reads the criterion strictly. Either get user approval that lands the sketch despite the temporal mismatch, or defer the sketch's `done` transition until after W1-T1.

- **Do NOT send 5+ parallel agents on docs that all reference the same epic doc and each other.** They produce isolated outputs with mutually inconsistent assumptions about each other's not-yet-finalised content (D2 hedged the namespace because D1a hadn't settled). Sub-wave 0a should have been D1a alone first, then D2/D3/D4/R3 in parallel after D1a was committed and re-readable. Cost ~1 rework cycle on D2.

## Open questions needing user input

See "Escalations" section above. Four blocking decisions.

## Reference artefacts

- Epic: `jit issue show ae82bd73`
- Epic design doc: `dev/plans/gf2_algebra_permanent.md` (modified in this session: §7.2 row-major → column-major + §13 W1 task table reflects same)
- Sub-wave 0a sketches:
  - `dev/plans/d1a_gf2_algebra_boundary.md` — D1a (6e20133d)
  - `dev/plans/d2_lean_bipedal3_sketch.md` — D2 (a0c0a45f)
  - `dev/plans/d3_lean_ryser_sketch.md` — D3 (4aaa6e4d)
  - `dev/plans/d4_intrinsic_feasibility.md` — D4 (4c534d31)
  - `dev/plans/r3_multi_word_streaming.md` — R3 (60c30e2d)
- D4 stub crate: `dev/research/intrinsic_feasibility_stub/`
- Progress: `dev/active/ae82bd73-progress.json`
- External references:
  - Scheinerman 2024 (arxiv 2407.20205v2): bipedal F_3 algorithm, paper Theorem 2.1, Table 2 baseline.
  - Hunter-Kwan-Sauermann 2026 (arxiv 2603.15856v1): theoretical companion for 8e4e19a0.
  - CLAUDE.md §Verification work: governs sketch-approval flow.
  - CLAUDE.md §Architecture / unsafe isolation: literal reading is what the reviewer enforces.
  - CLAUDE.md §Breakdown-time feasibility check: justifies D4's existence (afac2262 lesson).
- Gate-run artefacts: `.jit/gate-runs/{d0b1e9be,9e7be82d,1228b175,5542bddb,a4308e56,f2b4ee2b,7ce1fce2,9b703a49,c3d3d041}/result.json` — full reviewer verdicts for each round.

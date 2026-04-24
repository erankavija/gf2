# Handoff — Generic finite field linear algebra (bb85c68a) — session 2

**Date:** 2026-04-24
**Session number:** 2
**Prior handoffs:** `dev/active/bb85c68a-handoff.md` (session 1 — setup only)

## Current state

- Epic: `bb85c68a` — state: **backlog**, claimed by `agent:project-lead`.
- Wave in progress: **wave 1 complete**, wave 2 **not yet dispatched**.
- Children summary: **2 done** (ab791e27, cdcebf6a), 0 in_progress, 7 stories + 8 tasks = 15 items remaining, 0 rejected.
- Active claims: only `bb85c68a` itself (lead claim, indefinite TTL).
- Open escalations: None.
- Progress file: `dev/active/bb85c68a-progress.json` — updated to reflect wave 1 completion.

## What just happened

- Dispatched `ab791e27` (FieldMatrix foundation). 8 review cycles before PASS; final commit `b8bbfe6`. 32 reviewer findings closed.
- Escalated architectural ambiguity (zero-inner-dim `gemm`/`matvec`/`matvec_transpose` on runtime-context fields). User approved Option 4 — documented panic on `F: FiniteField` non-`ConstField`, with issue description amended to record the limitation.
- Identified process failure (reviewer drift + asymmetric rework prompts let gaps survive multiple rounds). User requested sharpening.
- Applied skill edits A–E to `~/.claude/skills/project-lead/`:
  - `lead-review-protocol.md`: added Tier 1.5 (prior-findings regression check) and Tier 2.75 (deferred-items audit); updated verdict template with both tables.
  - `rework-prompt-template.md`: added required pre-commit audit block (enumerate prior findings, list linked docs, grep for deferred-markers); restructured resolution table to span all rounds + design-doc deferrals.
  - `SKILL.md` Section 7: updated tier list with explicit "audit holistically on every round" requirement.
- Dispatched `cdcebf6a` (expression-template design). 3 review cycles before PASS; final commit `5840e5e`. Amended aspirational `cargo asm` criterion to defer empirical verification to `d48a3cfd/7e6183bb`. Added lead-approval note in doc header to break circular "approval before downstream implementation" gate.

## What to do next

- [ ] **Read the updated lead-review-protocol and rework-prompt-template before dispatching wave 2.** The new Tier 1.5 and 2.75 audits are mandatory on every review cycle. Failure to apply them is what turned a 2-cycle story into 8 cycles on `ab791e27`.
- [ ] **Wave 2 dispatch plan** (per `dev/active/bb85c68a-progress.json`):
   - **`d48a3cfd`** has child tasks — dispatch `91c06222` (T1: classical blocked gemm + eager operator overloads) first. Then `7e6183bb` (T2: expression-template proxies, follows `cdcebf6a`'s blueprint) AND `ad597ede` (T3: Strassen-Winograd + bound propagation) can run in parallel. Do NOT dispatch the story `d48a3cfd` itself — dispatch the tasks.
   - **`c3f8c1cb`** (PLE + echelon + LU + rank + nullspace). Depends on `ab791e27`. Implementation over `F: ConstField` (per approved Known Limitations). Worker must read Dumas-Pernet §2.2 algorithm 2.5.
   - **`83b1ad8b`** (trsm/trmm/trtri/trtrm block-recursive primitives). Depends on `ab791e27`. Worker reads Dumas-Pernet §2.1.
   - **`8a90882e`** (sparse CSR/CSC). Touches `field/sparse_matrix.rs` (disjoint from the dense path). Will replace the minimal stub that `ab791e27` introduced.
   - **Parallelism**: `d48a3cfd`/T1, `c3f8c1cb`, `83b1ad8b` all touch `field/matrix.rs` or declare new modules under `field/`. Serialize or worktree-isolate to avoid the "parallel agents reverting each other's WIP" trap. `8a90882e` is disjoint and safe to run in parallel.
- [ ] After each wave 2 worker, run the Tier 1.5 + Tier 2.75 audits from the new protocol **before** accepting PASS. Expect 1 cycle per story if the audit catches gaps upfront.
- [ ] Expect the reviewer to drift into new findings each round — this is a property of the AI reviewer, not a worker problem. The new protocol's Tier 1.5 makes the lead the memory across rounds.

## Traps — do not repeat these

**This section is mandatory.** Every trap below applies for the lifetime of the epic.

Carried forward from `bb85c68a-handoff.md` (session 1):
- PLE-first over LU.
- No unification of `BitMatrix` with `FieldMatrix<GF(2)>`.
- Dispatch tasks, not oversize stories (`d48a3cfd`, `64c88ae4`, `e47231cd`).
- `64c88ae4` runs in a pinned container — do NOT install fflas-ffpack/M4RI on host.
- Per-story criterion benches are `[hard]`, not best-effort.
- Don't re-add the transitively-redundant epic→{8a90882e, ae1d1e88} edges.
- Don't dispatch `d48a3cfd/T2` (7e6183bb) before `cdcebf6a` is done. **UPDATE: cdcebf6a is now done (2026-04-24). T2 unblocked.**
- SIMD foundation is complete (from `e095a100`) — don't add new SIMD kernels inside the matrix layer.
- Don't dispatch `16283d6f` (GPU epic) in this session.

New from session 2:

- **Reviewer drift is real.** The AI code-review gate samples a different subset of findings each round. Without Tier 1.5 (prior-findings regression check), gaps survive multiple rounds. Evidence: `ab791e27` took 8 cycles because finding-per-round was the pattern. Mitigation: always run the Tier 1.5 audit before accepting or dispatching rework.

- **Workers silently defer via their own design docs.** `ab791e27`'s worker wrote "Scalar `Mul<F>` for non-`Fp` fields: … deferred" in `dev/active/ab791e27-design.md` §8. I accepted that across 3 rounds. The reviewer caught it in round 5. Mitigation: Tier 2.75 deferred-items audit on every review.

- **Rework prompts must be symmetric.** My rework-1 prompt listed panic tests for `gemm` and `matvec` but omitted `matvec_transpose`. Worker followed literally; reviewer caught in round 4. When a class of thing needs testing, list all members of the class, not a subset.

- **Right-scalar `Mul<F>` is bounded on `F: FiniteField`** (`crates/gf2-core/src/field/matrix.rs:2154-2184`). Left-scalar `F * &M` is bounded per-type via the `impl_left_scalar_mul!` macro (stamped for `Fp<P>`, `GoldilocksFp`, `QuadraticExt<C>`, `CubicExt<C>`, `Gf2mWide<N, Cfg>`). Don't widen left-scalar to a blanket — orphan rule blocks it.

- **`with_capacity(rows, cols)` is bounded on `F: ConstField`** — fills with zeros. The Armadillo `fill::none` no-init optimization is not available without `unsafe` which `gf2-core` denies. The docstring and `armadillo_ux_mapping.md:20` are aligned with this resolved contract. Don't "optimize" it back to a 0×0 shell.

- **`gemm`/`matvec`/`matvec_transpose` zero-inner-dim corner panics on runtime-context fields** with a documented message. Tests at `crates/gf2-core/src/field/matrix.rs:2756-2875` lock this behavior. PLE (`c3f8c1cb`) must parameterize on `F: ConstField` to avoid hitting the panic on rank-0 inputs. Ae1d1e88 / e47231cd inherit the same bound.

- **`FiniteField::zero_hint() -> Option<Self>`** was added (returns `Some(F::zero())` for `ConstField` impls, `None` for `Gf2mElement`). This is the distinguisher that lets runtime-generic code decide whether it can fabricate a zero. Downstream stories should use it if they need a zero witness for runtime-context fields.

- **The `Transposed<M>` shell is minimal.** Full expression-template proxy algebra is designed in `dev/plans/expression_templates_design.md` (from `cdcebf6a`) and will be implemented by `d48a3cfd/7e6183bb`. That task should extend `Transposed<M>`, not replace it.

- **Design-doc "Follow-up work" bullets must cite issue Non-goals explicitly** or they'll be flagged as silent deferrals by the Tier 2.75 audit. `ab791e27-design.md` §8 and `expression_templates_design.md` §12 have been audited and comply; future design docs must do the same.

## Open questions needing user input

None. All session-2 questions are resolved. User approved:
- Option 4 (documented panic) for zero-inner-dim on runtime-context fields.
- Skill edits A–E applied to tighten the rework protocol.

## Reference artefacts

- Epic: `jit issue show bb85c68a`
- Progress file: `dev/active/bb85c68a-progress.json`
- Prior handoff: `dev/active/bb85c68a-handoff.md`
- Design docs:
  - `dev/plans/fflas_ffpack_analysis.md`
  - `dev/plans/dumas_pernet_takeaways.md`
  - `dev/plans/armadillo_ux_mapping.md`
  - `dev/plans/expression_templates_design.md` (NEW — from cdcebf6a)
  - `dev/active/ab791e27-design.md` (worker's design notes for ab791e27)
  - `dev/plans/gpu_fieldmatrix_sketch.md` (follow-on epic 16283d6f; not in scope)
- Updated skill files (in `~/.claude/skills/project-lead/`):
  - `SKILL.md` Section 7 (tier list)
  - `references/lead-review-protocol.md` (Tier 1.5, Tier 2.75)
  - `references/rework-prompt-template.md` (pre-commit audit, expanded sweeps, all-rounds resolution table)
- Key commits from this session:
  - `402208a` → `8f6a8ad` → `50bd4c5` → `398b0db` → `24c590a` → `c7d7ae4` → `fed2320` → `b8bbfe6` (`ab791e27` rework chain)
  - `1d284c6` (`ab791e27` done, JIT state)
  - `75a0af9` → `811e1fe` → `5840e5e` (`cdcebf6a` design + amendments)

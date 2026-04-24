# Handoff — Generic finite field linear algebra (bb85c68a) — session 3

**Date:** 2026-04-24
**Session number:** 3
**Prior handoffs:** `dev/active/bb85c68a-handoff.md` (session 1), `dev/active/bb85c68a-handoff-2.md` (session 2).

## Current state

- Epic: `bb85c68a` — state: **backlog**, claimed by `agent:project-lead`.
- Wave in progress: **wave 2, partially complete**.
- Children summary: **4 done** (ab791e27, cdcebf6a, 91c06222, 8a90882e), 0 in_progress, **1 stashed WIP** (7e6183bb), 12 remaining (8 stories + 4 tasks), 0 rejected.
- Active claims: only `bb85c68a` itself (lead claim, indefinite TTL).
- Open escalations: None.
- Progress file: `dev/active/bb85c68a-progress.json` — updated to reflect this session.
- **Critical resume artefact**: `git stash@{0}` — partial T2 (7e6183bb) work; see "What to do next" below.

## What just happened

- Confirmed wave 1 completion (ab791e27, cdcebf6a from session 2) and readiness of wave-2 entry points.
- Added missing dep edge `c3f8c1cb → 83b1ad8b` (PLE's description calls trsm a hard requirement but the DAG edge was absent). `jit validate --fix` removed 3 transitive redundancies post-add; DAG is clean.
- **Dispatched `91c06222` (d48a3cfd/T1: blocked gemm + eager operators).** Worker landed `17b6fa4` (blocked gemm using `FieldVec::dot_product_slices`, delayed reduction via `F::Wide` + kmax chunking, full operator surface, criterion bench for n=64/256/1024 on Mersenne-31 and GF(2^8)). `cargo-ci` PASS. `code-review` FAIL with one finding: missing `# Arguments`/`# Examples`/`# Complexity` sections on `pub fn gemm`. Rework commit `3aba44e` added the sections. `cargo-ci` + `code-review` PASS. State: **done**. Rework cycles: 1.
- **Dispatched `8a90882e` (sparse CSR/CSC FieldMatrix).** Worker landed `5a9a287` (full CSR + CSC, MatrixLike impls for both, from_triplets with sum-duplicates/drop-zeros/sort, SpMV/SpMVᵀ/SpMM cross-checked vs dense on 4 fields, criterion bench at densities 1%/5% on n=256/1024/4096 for Mersenne-31 and GF(2^8), module docs citing Dumas-Pernet §5 as out-of-scope). `cargo-ci` PASS. `code-review` FAIL with 2 findings: (1) parity table omitted `SpBitMatrix::col_iter` and `from_coo_deduplicated`; (2) `matvec` allocated `Vec<F>` per non-empty row. Rework commit `ba16868` fixed both + 5 sweep findings (8-row resolution table). `cargo-ci` + `code-review` PASS. Lead polish commit `aa1bbc5` fixed a 1-line parity-table wording (matvec_transpose "via CSC" → "via CSR scatter") that the reviewer had explicitly called "minor / not a fail". State: **done**. Rework cycles: 1.
- **Dispatched `7e6183bb` (d48a3cfd/T2: expression templates).** Worker began implementing — wrote **1934 lines of new `expr.rs`** (proxy taxonomy + trace counters + MatrixLike impls) plus a significant `matrix.rs` refactor (213 lines delta) and 32 lines added to `matrix_like.rs` — then **hit the model rate limit before reporting/compiling**. Partial work does NOT compile (blanket `impl<F, E> From<E> for FieldMatrix<F> where E: Evaluate<F>` at `expr.rs:1469` conflicts with `core`'s blanket `impl<T> From<T> for T` → E0119). Stashed as `stash@{0}` with the exact failure described in the stash message. Released the JIT claim and reset state to `ready`.
- Updated `dev/active/bb85c68a-progress.json` with session-3 status, rework counts, and new traps.

## What to do next

1. [ ] **Resume T2 (7e6183bb) from the stash.** Commands:
   ```bash
   cd /home/vkaskivuo/Projects/gf2
   git stash show stash@{0}               # verify it's the T2 WIP, not something else
   git stash apply stash@{0}              # bring the partial work back
   cargo check -p gf2-core --all-features # should still error with E0119 on expr.rs:1469
   ```
   Then fix the `From`/`Into` bridge per `dev/plans/expression_templates_design.md` §5. The blanket `impl<F, E> From<E> for FieldMatrix<F>` **does not work** — it conflicts with the core-provided `impl<T> From<T> for T`. Options:
   - (a) **Per-proxy `From` impls**: `impl<...> From<Product<A, B>> for FieldMatrix<F>`, `impl<...> From<Sum<A, B>>`, `impl<...> From<FusedProductPlus<...>>`, etc. One per proxy. Verbose but rule-compatible.
   - (b) **Inherent `FieldMatrix::eval<E: Evaluate<F>>(expr) -> Self`**: users write `FieldMatrix::eval(&a * &b + &c)` instead of `(&a * &b + &c).into()`. Loses `.into()` ergonomics but avoids the orphan/blanket issue entirely.
   - (c) **Typed-bridge wrapper**: a newtype `Expr<E>(E)` that users never write directly; the operator overloads already produce `Expr<E>`, and `impl<F, E> From<Expr<E>> for FieldMatrix<F>` is legal (no conflict, because `Expr<E>` is crate-local).

   The design doc should already have decided between (a)/(b)/(c). Read §5 first. If it doesn't decide, the next lead picks and documents the choice at the top of `expr.rs`.
   Also: clean up the stale `use std::ops::{Add, Bound, Index, Mul, Neg, RangeBounds, Sub};` in `matrix.rs:31` — the refactor made Add/Mul/Neg/Sub unused (rustc 4 warnings).

2. [ ] **Claim + dispatch T2 resume** via a *rework-shaped* prompt — the worker's work is 80% there, it needs a targeted finish prompt (fix the From/Into approach, finish whatever's half-done, run gates). Do NOT redispatch the original full prompt; the 1934 lines are mostly correct.

3. [ ] **Dispatch T3 (ad597ede: Strassen-Winograd)** once T2 is done. Can serialize after T2 completes; both touch `matrix.rs` but add disjoint surface areas.

4. [ ] **Transition d48a3cfd story to done** after T1+T2+T3 all done. The story is narrative-only; `jit issue update d48a3cfd --state done` (gates on the story are code-review + cargo-ci + doc-review; the tasks' gates don't auto-roll-up).

5. [ ] **Wave 2 remainder after d48a3cfd:** `83b1ad8b` (trsm/trmm/trtri/trtrm, depends on d48a3cfd); then `c3f8c1cb` (PLE, depends on 83b1ad8b).

6. [ ] **Waves 3 + 4**: per progress file.

## Traps — do not repeat these

**This section is mandatory.** Every trap below applies for the lifetime of the epic.

Carried forward from session 1 handoff — still binding:
- PLE-first over LU. No `BitMatrix`/`FieldMatrix<GF(2)>` unification. Dispatch tasks (not oversize stories) for `d48a3cfd`/`64c88ae4`/`e47231cd`. `64c88ae4` runs in a pinned container only — never `apt install libfflas-ffpack-dev` on host. Per-story criterion benches are `[hard]`. Don't re-add the transitively-redundant epic→{8a90882e, ae1d1e88} direct edges. SIMD foundation is done (epic `e095a100`) — no new SIMD kernels inside the matrix layer. GPU epic `16283d6f` is out-of-scope for this epic.

Carried forward from session 2 handoff — still binding:
- Reviewer drift is real; Tier 1.5 prior-findings regression check is mandatory on every re-review. Workers silently defer via their own design docs; Tier 2.75 is mandatory on every review. Rework prompts must be symmetric (list all members of a class, not a subset). Right-scalar `Mul<F>` bounded on `F: FiniteField`, left-scalar is per-`ConstField`-family (orphan rule). `with_capacity(rows, cols)` fills with zeros; the no-init optimization is not available without `unsafe` (denied in gf2-core). Zero-inner-dim `gemm`/`matvec`/`matvec_transpose` panic on runtime-context fields with a locked message (see `field/matrix.rs:2756-2875`). `FiniteField::zero_hint()` is the `ConstField`/non-`ConstField` distinguisher. `Transposed<M>` is a minimal shell; T2 extends it, does not replace. Design-doc "Follow-up work" bullets must cite issue Non-goals.

New from session 3:

- **`impl<F, E> From<E> for FieldMatrix<F> where E: Evaluate<F>` does NOT compile** — conflicts with core's universal `impl<T> From<T> for T` (E0119). The T2 worker walked into this trap; see stash@{0} line 1469. If you see this pattern in any future expression-template / proxy / wrapper code, reject it immediately. Legal alternatives listed in "What to do next" #1.

- **`jit_gate_pass` tool output is misleading on auto gates.** Calling `jit_gate_pass <issue> <gate>` returns `"Passed gate X for issue Y"` even when the underlying auto-runner (cargo-ci.sh, ai-review.sh) exited with failure. The only reliable check is `jit_gate_check-all <issue> --json` or direct inspection of `.jit/gate-runs/<run_id>/result.json`. Witnessed twice in session 3 (on 91c06222 commit `17b6fa4` and 8a90882e commit `5a9a287`). Mitigation: always run `jit gate check-all` after `jit gate pass` to verify the actual run status.

- **Serialize wave dispatches by default**, even when files are disjoint. Two reasons: CLAUDE.md forbids parallel `cargo` commands (shared `target/` cache), and `feedback_parallel_agent_isolation` warns against same-repo parallel agents reverting each other's WIP. Unless the project sets up git-worktree isolation, serial is the rule. Session 3 proved the overhead is tolerable (3-6 minutes per dispatch including gate runs).

- **Rework budget planning: 1 round per dispatched issue** is the observed baseline. Both 91c06222 and 8a90882e needed exactly 1 rework round, driven by the doc-contract (missing `# Arguments`/`# Examples`/`# Complexity`). Preemptively emphasize the CLAUDE.md doc standards in every implementation dispatch prompt — and maybe sweep for them in a dispatch precheck to catch at commit-time rather than review-time.

- **Worker's design docs can contradict their own parity tables** (8a90882e session 3: table said `matvec_transpose` "via CSC", impl used CSR scatter). The reviewer may flag this as "minor, not a fail" — but the lead should still fix it, because the next consumer of the doc (PLE, inv) will be misled. A 1-line polish commit from the lead is the right action here.

## Open questions needing user input

- Question: Should the next session's T2 resume use rework-prompt (targeted) or a fresh full dispatch?
  - Context: 1934 lines of expr.rs + significant matrix.rs refactor are stashed. Most of it is probably correct; the From/Into bridge needs a structural fix. A full redo discards ~6 hours of work; a targeted rework preserves it but requires the next worker to read and understand the stash.
  - Options: (a) targeted rework from stash; (b) discard stash, fresh dispatch; (c) lead fixes From/Into inline, then spot-dispatches for remaining gaps.
  - Recommendation: **(a) targeted rework from stash.** The design doc's §5 is clear enough that a worker can read it and fix the bridge without restarting.

## Reference artefacts

- Epic: `jit issue show bb85c68a`
- Progress file: `dev/active/bb85c68a-progress.json`
- Prior handoffs: `dev/active/bb85c68a-handoff.md`, `dev/active/bb85c68a-handoff-2.md`
- Stashed T2 WIP: `git stash show -p stash@{0}` (1934 lines expr.rs + 213-line matrix.rs delta + 32-line matrix_like.rs delta)
- Design docs for T2 resume:
  - `dev/plans/expression_templates_design.md` (1033 lines; §5 is the From/Into bridge)
  - `dev/plans/dumas_pernet_takeaways.md` §1.2 (delayed reduction)
  - `dev/active/ab791e27-design.md` §2 (Transposed<M> current shape)
- Session-3 commits:
  - `17b6fa4` → `3aba44e` (91c06222 main + rework)
  - `5a9a287` → `ba16868` → `aa1bbc5` (8a90882e main + rework + polish)

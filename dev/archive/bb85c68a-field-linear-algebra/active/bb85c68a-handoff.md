# Handoff — Generic finite field linear algebra (bb85c68a) — session 1

**Date:** 2026-04-23
**Session number:** 1
**Prior handoffs:** None.

## Current state

- Epic: `bb85c68a` — state: **backlog**, claimed by `agent:project-lead`.
- Wave in progress: **none** — Wave 1 not yet dispatched. Lead was asked to assess readiness only; dispatch is on user command.
- Children summary: **0 done, 0 in_progress, 9 stories + 8 tasks = 17 items pending**, 0 rejected.
- Active claims: only `bb85c68a` itself (lead claim, indefinite TTL, ~30 min old at time of handoff).
- Open escalations: None.
- Progress file: `dev/active/bb85c68a-progress.json` — reflects the above, includes full task breakdown and trap log.
- **Uncommitted state (critical for resume):** all `.jit/` mutations from this session, plus all new `dev/plans/*.md` + `dev/active/*.json` + this handoff, are currently uncommitted. The next session will need them in git before dispatching, or it risks re-running JIT operations that were already done.

## What just happened

Wave 0 (setup-only). No code written; no workers dispatched.

- Read epic `bb85c68a` + its 3 direct children; traversed to transitive children (6 stories total originally).
- Fetched `dev/plans/fflas_ffpack_analysis.md` for prior context; fetched arXiv:1204.3735 (Dumas-Pernet) via `pdftotext`; read §§1–3 in detail.
- Interviewed user on 4 scope forks; user chose: **name-parity + shared trait** (not unification), **PLE-first** (not PLU), **expression templates** (not minimal), **all three add-ons** (trsm, per-story benches, M4RI, char poly).
- Created design docs: `dev/plans/dumas_pernet_takeaways.md`, `dev/plans/armadillo_ux_mapping.md`. Attached both to `bb85c68a`.
- Created 3 new stories: `cdcebf6a` (expression-template design), `83b1ad8b` (trsm/trmm/trtri/trtrm), `e47231cd` (char/min poly). All gated with code-review + cargo-ci + doc-review.
- Rewrote scope of 6 existing items: `ab791e27` (MatrixLike + Armadillo parity), `c3f8c1cb` (PLE-first), `ae1d1e88` (on PLE + trsm), `64c88ae4` (+M4RI, container-pinned), `d48a3cfd` (+ proxy-type scope), `8a90882e` (naming parity with `SpBitMatrix`). Epic description rewritten with explicit Success Criteria + Child Stories table.
- Attached `code-review + cargo-ci + doc-review` gates to the epic and every story (7 issues).
- Wired inter-story deps; epic now depends on `sparse, bench, inv, charpoly` (transitively all Wave 4 predecessors).
- Created GPU follow-on epic `16283d6f` (HIP, dense matmul + SpMV) with its own design doc `dev/plans/gpu_fieldmatrix_sketch.md`; state backlog, priority low, depends on bb85c68a + 64c88ae4.
- User asked "are the stories small enough?" → identified `d48a3cfd`, `64c88ae4`, `e47231cd` as oversize.
- User chose breakdown via child tasks (not story splits).
- Created 8 tasks across the 3 oversize stories: `91c06222/7e6183bb/ad597ede` (d48a3cfd), `a03b2556/6ed7f050/a9ab0a4f` (64c88ae4), `f01298db/1454ec2d` (e47231cd). Tasks carry `code-review + cargo-ci`; `a9ab0a4f` also carries `doc-review` since it delivers the bench results doc.
- Wired intra-task deps + story ← task deps.
- `jit validate --fix` removed 2 transitively-redundant epic-level edges (`bb85c68a → 8a90882e` and `bb85c68a → ae1d1e88`) — they are still reachable through `bb85c68a → 64c88ae4 → 6ed7f050 → {8a90882e, ae1d1e88}`. DAG integrity preserved.
- Updated `dev/active/bb85c68a-progress.json` to include the task breakdown and gate-policy notes.

## What to do next

1. [ ] **Commit the session's state first.** `git add .jit/ dev/plans/ dev/active/bb85c68a-progress.json dev/active/bb85c68a-handoff.md && git commit`. Without this commit, the DAG changes and progress file are invisible to a fresh shell, and you will re-run work.
2. [ ] Confirm with the user that Wave 1 dispatch is authorized (the prior session stopped at readiness assessment per user's literal ask — dispatch was not approved).
3. [ ] Run `jit recover` (cleans any stale locks from the prior session) and `jit validate` (should pass).
4. [ ] **Wave 1 dispatch plan:**
   - Dispatch `ab791e27` first (design dense `FieldMatrix` + `MatrixLike<Elem>` trait). Classification: `design`. Use `references/architect-agent-prompt.md`.
   - When `ab791e27` passes review + gates, dispatch `cdcebf6a` (design expression templates). `cdcebf6a` depends on `ab791e27` since proxy signatures reference `FieldMatrix` + `MatView`. Serialized within Wave 1.
5. [ ] After Wave 1, advance to Wave 2 per `bb85c68a-progress.json`. Remember: for `d48a3cfd` in Wave 2, dispatch the **tasks** (`91c06222` first, then `7e6183bb` and `ad597ede` in parallel), not the story itself.

## Traps — do not repeat these

**This section is mandatory.** Every trap below applies for the lifetime of the epic.

- **Do NOT treat LU-with-partial-pivoting as the core factorization.** Over finite fields, PLE (Permutation · Lower · Echelon) is the ground truth — Dumas-Pernet §2.2 algorithm 2.5. Story `c3f8c1cb` now specifies PLE-first; LU is derived. If a worker returns a PR that implements a bespoke LU loop, reject it. Evidence: Dumas-Pernet §2.2, `dev/plans/dumas_pernet_takeaways.md` §3.1.

- **Do NOT unify `BitMatrix` with `FieldMatrix<GF(2)>`.** User chose "Name parity + shared trait", not "Full unification". `BitMatrix` keeps bit-packed storage and its existing hot paths. The shared `MatrixLike<Elem>` trait is what creates API consistency — not a merge. Evidence: user's answer to the API-parity question in session 1; `dev/plans/armadillo_ux_mapping.md` §3.

- **Do NOT dispatch the three broken-down stories directly — dispatch their child tasks.** `d48a3cfd`, `64c88ae4`, `e47231cd` now depend on child tasks. The story issue is a narrative wrapper; the actual work is in the tasks. Dispatching the story will either confuse the worker (too much scope) or be invalid (story's deps aren't done yet). Evidence: the task breakdown was added precisely because stories were oversize for single-session dispatch.

- **Do NOT re-add the `bb85c68a → 8a90882e` or `bb85c68a → ae1d1e88` direct edges.** `jit validate --fix` removed them because they are reachable transitively through `bb85c68a → 64c88ae4 → 6ed7f050 → {8a90882e, ae1d1e88}`. The DAG still enforces that the epic cannot close until those stories are done. If validation complains again, run `jit validate --fix`; do not hand-edit.

- **Do NOT let per-story criterion benchmarks become best-effort.** User stated benchmarking is mandatory; criterion micro-benches are a `[hard]` success criterion at every implementation story, not only at `64c88ae4`. A PR that lands matmul / PLE / sparse / inv / charpoly without benches is a FAIL on `code-review`. Evidence: user's opening prompt; epic description success criteria.

- **Do NOT install fflas-ffpack or M4RI on the bare host.** User chose container-pinned methodology. `64c88ae4/T1` builds them inside a Debian-slim image. A worker who tries `apt install libfflas-ffpack-dev` is going wrong. Evidence: user's answer to the fflas-ffpack question; `64c88ae4/T1` description.

- **Do NOT dispatch `d48a3cfd/T2` before `cdcebf6a` (expression-template design) is reviewed and marked done.** T2 implements the proxy types cdcebf6a specifies. If T2 goes first, it will invent its own ad-hoc proxy scheme that disagrees with everything else. Evidence: T2's dependency on `cdcebf6a` + `91c06222`.

- **Do NOT assume SIMD needs new work in this epic.** The CPU SIMD foundation (AVX2 / AVX-512 IFMA / PCLMULQDQ, `FieldBackend` trait, runtime `OnceLock` dispatch) was delivered by `e095a100` and is load-bearing here. `FieldMatrix` inherits SIMD through `FieldVec::dot_product`. Do not accept a worker's PR that adds new SIMD kernels inside the matrix layer — those belong in `gf2-kernels-simd` if needed at all. Evidence: `CLAUDE.md` architecture section; `e095a100` completion report.

- **Do NOT dispatch `16283d6f` (GPU epic) as part of this session.** It is backlog, pre-filed, depends on `bb85c68a`. Wait for the CPU epic to complete and the user to explicitly authorize dispatch. Evidence: `16283d6f` description's Status block.

## Open questions needing user input

- Question: Wave 1 dispatch authorization.
  - Context: Session 1 stopped at readiness assessment per user's literal ask. User has not yet said "go".
  - Options: (a) Dispatch Wave 1 now; (b) Hold for user directive.
  - Recommendation: **Hold.** User's last visible prompt asked about readiness, not execution. Confirm before dispatching.

- Question: Worktree isolation for Wave 2.
  - Context: `d48a3cfd`, `c3f8c1cb`, `83b1ad8b` all touch `crates/gf2-core/src/field/*`. Parallel same-repo dispatch triggers `feedback_parallel_agent_isolation` (memory: workers can revert each other's WIP).
  - Options: (a) Serialize within Wave 2 (safer, slower); (b) Worktree mode (parallel, isolated); (c) Mix — some parallel, some serial.
  - Recommendation: **(b) Worktree mode** if JIT's `[worktree]` config supports it; otherwise **(a) serialize**. Defer decision to wave 2 start.

## Reference artefacts

- Epic: `jit issue show bb85c68a`
- Progress file: `dev/active/bb85c68a-progress.json`
- Design docs:
  - `dev/plans/fflas_ffpack_analysis.md` (library analysis, pre-existing)
  - `dev/plans/dumas_pernet_takeaways.md` (arXiv:1204.3735 → our epic)
  - `dev/plans/armadillo_ux_mapping.md` (Armadillo → `FieldMatrix`, `MatrixLike` trait, expr templates)
  - `dev/plans/gpu_fieldmatrix_sketch.md` (follow-on epic `16283d6f`)
- Follow-on (not in this epic): `jit issue show 16283d6f`
- Completed foundation: `jit issue show e095a100` (`FieldVec<F>`, `Wide`, SIMD, formal verification)
- External refs (user-authorised): https://arxiv.org/abs/1204.3735, https://arma.sourceforge.net/, https://github.com/linbox-team/fflas-ffpack

# Handoff — Generic finite field linear algebra (bb85c68a) — session 5

**Date:** 2026-04-26
**Session number:** 5
**Prior handoffs:** `bb85c68a-handoff.md` (s1), `-handoff-2.md` (s2), `-handoff-3.md` (s3), `-handoff-4.md` (s4).

## 1. Current state

- Epic: `bb85c68a` — state: **backlog**, claimed by `agent:project-lead`.
- Wave 2 is **fully complete except c3f8c1cb (PLE)**.
- Wave 3 (`ae1d1e88` inv/solve/det, `e47231cd` charpoly) and Wave 4 (`64c88ae4` benchmark) **not yet started**.
- Active claims: `bb85c68a` (lead), `c3f8c1cb` (agent:claude — held since this session's start).
- **Open R4 review failure on `c3f8c1cb`**; reviewer flagged `rref()` for bespoke back-substitution instead of routing through `trtri`/`trtrm`. See §3.

### Issue map at handoff

| Issue | State | Notes |
|---|---|---|
| `ab791e27` | done | Wave 1 (s2) |
| `cdcebf6a` | done | Wave 1 (s2) |
| `91c06222` (T1) | done | s3 |
| `7e6183bb` (T2) | done | s4, 6 commits, runtime-speedup [aspirational] amendment |
| `ad597ede` (T3) | done | s4–s5, 6 review cycles, GF(2^8) n=4096 Criterion 15h overnight run |
| `d48a3cfd` story | done | s5; criterion fflas-ffpack tag amended [aspirational] |
| `8a90882e` (sparse) | done | s3 |
| `83b1ad8b` (triangular) | done | s5, **5 review cycles** — established the architectural pattern (views all the way down, gemm kernels for all matrix multiplication, allocation budgets locked in regression tests) |
| `c3f8c1cb` (PLE) | **in_progress** | s5, **7 review cycles** so far — see §3 for the open architectural finding |
| `ae1d1e88` (inv/solve/det) | pending | depends on PLE done |
| `e47231cd` (charpoly) | pending | depends on PLE done |
| `64c88ae4` (benchmark) | pending | terminal story; container-pinned |

### Commits landed in session 5 for c3f8c1cb (in chronological order)

| SHA | Round | Notes |
|---|---|---|
| `74867c3` | R0 (initial) | Clone-heavy; 35 allocs at n=4 |
| `bbf50d3` | n/a (state) | R1 review-fail recorded |
| `16639d3` | n/a (planning) | Replanning artefact `dev/active/c3f8c1cb-ple-replan.md` written + attached to issue |
| `6df73ad` | R1 rework | View-based driver + materialise helpers; 14/254/4192 alloc budget pinned |
| `bbc0bff` | R2 (description amend) | `[hard]` zero-alloc clarified to "no PLE-managed scratch beyond what `#![deny(unsafe_code)]` and the kernel B-transposes intrinsically allocate" |
| `e34a761` | R3 fix | `m × 0` zero-width edge case in `row_echelon` |
| `8ddde5a` | R4 fixes | `lu()` permutation orientation (was wrong for non-involutive perms — now `p_ple.inverse()`); independent rank construction test; rref panics doc |
| `4522b17` | R5 (description amend) | `E` carries pivot values, NOT unit leading coefficients (mathematically inconsistent with L-unit-diag inside `A = P · L · E`) |

## 2. What just happened in session 5 (chronological)

1. **Closed `83b1ad8b` (triangular primitives)** after 5 review cycles. Final commits: 748d455, be5d996, b3729aa, 8e02195, 0dac265, e4deb44. Established the architectural pattern that every subsequent issue on this epic must follow:
   - Recursion uses `MatViewMut` views; no `clone_block` / `clone_columns` helpers.
   - All matrix multiplication routes through `gemm_into_view` / `gemm_axpy_into_view` / `gemm_axpy_into_view_diag`.
   - The kernels' intrinsic B-transpose allocations are **accepted and documented**, NOT eliminated.
   - Trtri/trtrm allocate ONE chain-multiply scratch per recursion level — mathematical, not API.
   - Allocation regression tests pin exact integer counts (no loose windows).
   - Description amendments are user-approved and recorded in-line.

2. **Closed `d48a3cfd` (story-level)** via gate runs on the leaf-task commits + `[aspirational]` amendment to the fflas-ffpack-comparison criterion (deferred to `64c88ae4`).

3. **Dispatched `c3f8c1cb` (PLE)**. R0 (commit `74867c3`) failed R1 review on the same SSOT/allocation pattern that took triangular 5 cycles. User directed a **proper replanning round** before R1 rework. Replanning artefact written and attached: `dev/active/c3f8c1cb-ple-replan.md` (commit `16639d3`).

4. **Drove R1 → R5 on c3f8c1cb** through rework + lead-polish + 2 description amendments. Each cycle closed the prior reviewer findings cleanly but exposed a new narrower interpretation. Currently at **R5 verdict FAIL** on a new finding (see §3).

## 3. Open issue: c3f8c1cb R5 review verdict

R5 reviewer's verdict on commit `4522b17`:

> **FAIL — Single source of truth.** This is not a new GEMM/TRSM reimplementation, but it does create a parallel high-level triangular-elimination path instead of routing derived ops through the shared triangular primitives the codebase already provides. `rref()`'s manual elimination block in `ple.rs:894-927` is the clearest example.

Specifically: the issue description says

```
pub fn rref(&self) -> (FieldMatrix<F>, FieldMatrix<F>);
// Reduced row echelon form Y·A = R (alg 2.7). Uses PLE + trsm + trtri + trtrm.
```

The current implementation does back-substitution column-by-column with bespoke loops at `ple.rs:894-927` instead of composing `row_echelon` (PLE + `trsm_lower` + `trtri_lower`) with a `trtrm`-based above-pivot zeroing. That violates the strict reading of the SSOT contract.

Other R5 findings (`row_echelon` and `rref` not actually using triangular primitives end-to-end, doc claims internally consistent but not aligned with the issue spec) all reduce to the same root cause.

### Three options for the next session

1. **Refactor `rref()` to compose `trtri_lower(L_top)` + `trtrm` for the above-pivot zeroing.** Mechanically straightforward but the back-substitution that's currently bespoke needs to be expressed as `trtrm(L_top_inv, E)` or similar, requiring careful indexing through pivot columns. Estimated: 1–2 cycles. This is what the reviewer literally asks for.

2. **Amend the issue description (R6) to allow rref's bespoke back-substitution.** Same template as R2/R3/R5 amendments (architectural / mathematical reality). Justification: the bespoke loop at `ple.rs:894-927` is at most O(rank · n) field operations, dwarfed by PLE's O(min(m,n)·m·n); refactoring through `trtrm` would force an `m × m` materialisation of `L_top_inv` (an extra O(m²) allocation) for cosmetic SSOT compliance. Trade real performance for a SSOT box-tick.

3. **Stop iterating, file a follow-on issue, and close `c3f8c1cb` with a documented gate-failure exception.** The user previously said "performance contract is non-negotiable" — but option 2 IS about performance (avoiding a redundant `m × m` allocation). Option 3 is the cleanest path to unblock Wave 3 if the user prefers progress over gate-cleanliness here.

**Lead recommendation: option 1.** The refactor is bounded, the reviewer's reading is consistent with the issue text, and finishing the SSOT story matches the architectural rule we set in `83b1ad8b`. Option 2's amendment chain is starting to feel like death-by-a-thousand-cuts. Option 3 leaves real debt downstream stories will re-trigger.

But this is a user decision. Recommend: next session opens with `AskUserQuestion` presenting these three options with the same analysis above.

## 4. What to do next (priority order)

1. **Resolve c3f8c1cb R5** per user choice between options 1/2/3 above.
2. **Close c3f8c1cb story-level** (cargo-ci ✓, code-review must pass, doc-review = lead).
3. **Dispatch `ae1d1e88`** (inverse/solve/det). Depends on PLE + trsm. Should be straightforward by composition; expect ~1 review cycle if the worker reads the dispatched scope carefully.
4. **Dispatch `e47231cd`** (charpoly/minpoly). Two child tasks (`f01298db` cubic Krylov + `1454ec2d` Keller-Gehrig). Cubic baseline first; sub-cubic second. Workshops a research issue around finite-field charpoly per Dumas-Pernet §3.
5. **Dispatch `64c88ae4`** (benchmark). Three child tasks: container image (a03b2556), gf2 criterion suite (6ed7f050), published side-by-side analysis (a9ab0a4f). Container is the long pole — fflas-ffpack + M4RI in pinned Debian-slim image.
6. **Epic close**. Run epic-level gates, write completion report, transition `bb85c68a` to done.

Estimated remaining session count: 3–5 sessions.

## 5. Traps — do not repeat these

**Carried forward from sessions 1–4 (still binding):**

- PLE-first over LU. No `BitMatrix`/`FieldMatrix<GF(2)>` unification. Dispatch tasks (not oversize stories) for `d48a3cfd`/`64c88ae4`/`e47231cd`. `64c88ae4` runs in a pinned container only — never `apt install libfflas-ffpack-dev` on host. Per-story criterion benches are `[hard]`. SIMD foundation is done (epic `e095a100`) — no new SIMD kernels inside the matrix layer. GPU epic `16283d6f` is out-of-scope.
- Reviewer drift is real; Tier 1.5 prior-findings regression check is mandatory. Workers silently defer via design docs; Tier 2.75 is mandatory. Rework prompts must be symmetric.
- `jit_gate_pass` MCP call has a 10-min tool-level timeout; long ai-review runs need `Bash(jit gate pass <issue> code-review, run_in_background=true, timeout=900000)`.
- Serialize wave dispatches by default. CLAUDE.md forbids parallel `cargo` commands.
- AI reviewer may run its own local benches/tests to verify hard claims — worker self-reported numbers can be inverted under different settings.

**New from session 5:**

- **Architectural pattern from `83b1ad8b` is now load-bearing** for every dense-linear-algebra issue. Future workers MUST start by reading the triangular module's rustdoc to understand the conventions, OR the lead must include the conventions in the dispatch prompt verbatim. Otherwise the same 5-cycle drift happens.
- **Replanning rounds work.** When the first review of a new issue fails on the same architectural pattern that took 5 cycles to land before, IMMEDIATELY trigger a replanning round (write an artefact, attach to issue, reference in the next dispatch prompt). This was done for `c3f8c1cb` (commit `16639d3` artefact at `dev/active/c3f8c1cb-ple-replan.md`) and saved several cycles vs. blind iteration.
- **Description amendments are now a recognised tool.** Three categories of amendment have set precedent in this epic:
  - **`[hard]` → `[aspirational]` with measured evidence** (T2 R2: runtime-speedup at n=1024).
  - **Architectural-cost clarification** (T3 R5: trtri/trtrm chain-multiply scratch; PLE R2: `#![deny(unsafe_code)]` materialise helpers).
  - **Mathematical-inconsistency resolution** (PLE R3: E carries pivots, not unit leading; the original spec was self-contradictory inside `A = P·L·E`).
  Each is documented in-line in the issue description with the reasoning, the empirical evidence (where applicable), and a cross-reference to the precedent.
- **Reviewer's "narrowing interpretations" are a stable pattern.** Each rework cycle on a tricky issue tends to surface progressively narrower readings of the contract. The lead's job is to either (a) close the new finding cleanly, (b) amend the criterion if the finding is contractually impossible, or (c) declare ROI exhaustion and hand off / escalate.
- **Allocation-counter must be `thread_local`.** The 83b1ad8b R5 fix to `FIELDMATRIX_NEW_COUNT` (`matrix.rs:36-66`) is what makes the alloc-budget regression tests robust under both `cargo test` and `cargo nextest`. Future allocation-counting code in this epic should use the same pattern.
- **`#![deny(unsafe_code)]` is a real architectural cost.** PLE R2 amendment documents it: `MatViewMut::split_cols_mut` would let the recursion avoid materialising L1/L1_bot, but row-major strided splitting needs `unsafe`. The cost is recorded but not paid.
- **Repeated cycles on the same issue indicate a contract bug, not implementation skill.** When a worker has been through 3+ cycles and the reviewer keeps finding new things, it's almost always a sign that the contract has hidden tensions (the unit-leading × unit-diag math conflict in PLE; the zero-alloc × kernel-B-transpose tension in trsm/trmm). Lead intervention: trigger replanning, amend the contract, or accept ROI exhaustion.

## 6. Reference artefacts

- Epic: `jit issue show bb85c68a`
- Progress file: `dev/active/bb85c68a-progress.json` (will need updating to record session 5 — note: I did NOT update it this session because the session was driven by serial issue-completion and the file was already accurate after session 4's wave-2 closure work; the next session should refresh it before dispatching wave 3).
- PLE replanning artefact: `dev/active/c3f8c1cb-ple-replan.md` (commit `16639d3`).
- Prior handoffs: `bb85c68a-handoff{,-2,-3,-4}.md` — read in order, traps section in each.
- Key recent commits (HEAD-ish):
  - `4522b17` chore(jit:c3f8c1cb): R3 amendment — E carries pivot values
  - `8ddde5a` fix(jit:c3f8c1cb): R3 review fixes — lu() perm + tests + docs
  - `e34a761` fix(jit:c3f8c1cb): row_echelon m×0 edge case
  - `bbc0bff` chore(jit:c3f8c1cb): R2 amendment — materialise pattern
  - `6df73ad` fix(jit:c3f8c1cb): R2 rework — view-based PLE per replan
  - `16639d3` docs(jit:c3f8c1cb): replanning artefact
  - `bbf50d3` chore(jit:c3f8c1cb): R1 review FAIL recorded
  - `74867c3` feat(jit:c3f8c1cb): PLE decomposition + echelon forms (R0)
  - `cf3ee7c` chore(jit:83b1ad8b): triangular primitives done after 5 cycles
  - `e4deb44` fix(jit:83b1ad8b): R5 rework — alloc-free trtri base case
  - `0dac265` fix(jit:83b1ad8b): R4 rework — gemm_axpy_into_view_diag

## 7. Closing remark

Session 5 completed two more wave-2 stories (`d48a3cfd` after closing T3, `83b1ad8b` triangular) and pushed PLE through 7 review cycles. The architectural pattern is now well-established and codified across `83b1ad8b` and the PLE work; future issues should converge faster (1–2 cycles) provided the dispatch prompt encodes the pattern upfront.

The remaining wave-2 deliverable (PLE) has all its `[hard]` mathematical content correct and verified; the remaining cycle is about whether `rref()`'s bespoke back-substitution should be refactored through `trtri`/`trtrm` per literal contract reading (option 1 in §3). Recommend opening the next session with `AskUserQuestion` between options 1/2/3.

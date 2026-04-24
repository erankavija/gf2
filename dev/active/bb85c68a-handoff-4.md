# Handoff — Generic finite field linear algebra (bb85c68a) — session 4

**Date:** 2026-04-24 (evening)
**Session number:** 4
**Prior handoffs:** `bb85c68a-handoff.md` (session 1), `bb85c68a-handoff-2.md` (session 2), `bb85c68a-handoff-3.md` (session 3).

## 1. Current state

- Epic: `bb85c68a` — state: **backlog**, claimed by `agent:project-lead`.
- Wave in progress: **wave 2, partially complete**.
- Children summary: **4 done** (ab791e27, cdcebf6a, 91c06222, 8a90882e), **1 in_progress** (7e6183bb — T2), 11 remaining (8 stories + 3 tasks), 0 rejected.
- Active claims: `bb85c68a` (lead) + `7e6183bb` (agent:claude; worker hit rate limit mid-dispatch near end of session).
- Open escalations: **ONE OPEN — T2 runtime-speedup criterion; see §3.**
- Progress file: `dev/active/bb85c68a-progress.json` — updated.
- **Critical resume artefact:** none — no stash, session-4 work is committed as `e94eb23`.

## 2. What happened in session 4

Session 4 drove issue `7e6183bb` (d48a3cfd/T2: expression-template proxy types) through **3 full code-review cycles**:

| Round | Commit(s) | cargo-ci | code-review | Findings | Verdict |
|-------|-----------|----------|-------------|----------|---------|
| R1 worker initial | `bf78239` | ✓ | ✗ | F1 thread-unsafe trace-counter tests, F2 missing αAᵀ·B+βC, F3 no allocation evidence, F4 no evaluation-time panic tests, F5 stale `Evaluate<F>` docstring (+ lead find F6 `from_raw_parts_expr` redundancy) | FAIL |
| R2 worker rework | `a2c5cbb` | ✓ | ✗ | G1 `kernel_counts` overclaims parallel safety, G2 bench md overstates eager allocation count + axpy scratch | FAIL |
| R2 lead polish | `c267f25` | ✓ | ✗ | H1 **fused is empirically SLOWER than eager at n=1024** (fused 3.375 s vs eager 3.165 s in reviewer's Criterion run; lead verified with 20-sample run: fused 3.375 s, eager 3.165 s) | FAIL |
| R3 kernel rewrite (partial) | `e94eb23` | ✓ (tests pass) | not re-run (criterion still not met) | Fused now 3.227 s — 4.4% faster than R2 but eager is 3.163 s, **still ~2% slower**. | (blocked) |

User directive before R3 dispatch: **"Rewrite the kernel in T2 scope"**. The worker replicated T1's blocked structure (GEMM_ROW_TILE × GEMM_COL_TILE tiling + B-transpose + delayed reduction) across all three fused kernels (`gemm_with_beta_concrete`, `gemm_trans_a_concrete`, `gemm_trans_a_with_beta_concrete`). All 24 fusion/bit-exact/panic tests pass under `cargo nextest run --release --profile ci`. Clippy and fmt are clean. But the empirical benchmark at n=1024 still shows eager winning by ~2%.

**Why blocking alone cannot close the gap (physics):**
- Fusion's theoretical saving at n=1024 over Mersenne-31 is **~4 MB of one-off allocation + ~8 MB of avoided memory traffic** (intermediate matrix read + written once).
- Workload is **~10⁹ field multiplications** in the O(n³) dot-product — ~3 seconds of compute.
- Memory-traffic saving at DRAM bandwidth ~20 GB/s is ~0.4 ms; allocation is ~1 ms. Sum ≤ **~0.05%** of runtime.
- Eager's `&t + &c` axpy is almost free (one cache-resident pass over 4 MB, ~1 ms).
- Conclusion: without SIMD vectorisation inside the inner loop (or Strassen recursion moving below O(n³)), fused cannot meaningfully beat eager at this scale. The kernel rewrite is a strict improvement (prior fused was 3.375 s; now 3.227 s), but the 2% residual gap is at the noise floor.

All other R1/R2/R3 code-level findings are closed at HEAD:
- F1 serial-test isolation via `serial_test = "3"` + `#[serial]` annotations.
- F2 `ScaledTransposedProduct` proxy + full αAᵀ·B+βC fusion with evaluator, tests, and design-doc §5.4 amendment.
- F3 `benches/field_matrix_fusion_results.md` + `test_fused_path_allocates_fewer_matrices_than_eager`.
- F4 evaluation-time `#[should_panic]` tests cover all concrete-kernel `shape mismatch` messages.
- F5 `Evaluate<F>` trait docstring rewritten at `expr.rs:215-241`.
- F6 `from_raw_parts_expr` removed; all 5 call sites use `from_raw_parts` directly.
- G1 `kernel_counts` doc now names the `#[serial]` requirement instead of claiming robust deltas.
- G2 bench md says "2 matrices eager / 1 matrix fused / save 1 allocation" honestly.

## 3. Open escalation — user must pick one

### Option A (Recommended): mark runtime-speedup portion `[aspirational]` with session-4 evidence

**Scope change request:** amend the task's `## Success criteria` entry

> - [hard] Fused-vs-naive benchmark comparison present and shows the expected speedup.

to the split form

> - [hard] Fused-vs-naive benchmark comparison present; fused path allocates fewer matrices (verified at runtime via `test_fused_path_allocates_fewer_matrices_than_eager`).
> - [aspirational] Fused path measurably faster than eager at n=1024. Amendment (d48a3cfd/T2 R3, 2026-04-24): at n=1024 over Mersenne-31, blocked-kernel fused is 3.227 s vs eager 3.163 s (~2% slower). The fusion's theoretical saving (~0.05% of runtime at n=1024) is below Criterion's noise threshold. A speedup becomes tractable only with SIMD vectorisation inside the field-element inner loop or Strassen recursion reducing the O(n³) cost — both out of scope for T2. See `benches/field_matrix_fusion_results.md` for the measured evidence.

Then update `benches/field_matrix_fusion_results.md` with honest numbers and rerun `code-review`. Expect PASS.

This is what CLAUDE.md's `[aspirational]` marker semantics were designed for: "Written optimistically before empirical evidence existed; may be amended in-loop with observed number and reason."

### Option B: Defer the win to a new story (e.g., "SIMD/Strassen fused kernel")

Close T2 with the allocation-win criterion met and leave the runtime criterion unmet. File a new child story under `d48a3cfd` or as a follow-on to `ad597ede` (T3 Strassen-Winograd) that explicitly owns the fused-vs-eager runtime win. Benefit: T2 closes; real issue tracked. Cost: epic-level `## Child stories` table needs updating; adds a wave.

### Option C: Reject T2 and fold runtime-win into T3 (ad597ede Strassen-Winograd)

Mark `7e6183bb` as `rejected` with reason "runtime-win deferred to ad597ede", keep commits `bf78239`/`a2c5cbb`/`c267f25`/`e94eb23` as merged code, and amend T3's scope to explicitly require "fused vs eager at n=1024 must be faster after Strassen recursion". Benefit: cleanest DAG state. Cost: T3 grows; timing of fused-win proof depends on Strassen completion.

### Session-4 lead recommendation: Option A.

Rationale:
1. Criterion amendment with measurement evidence is exactly the mechanism CLAUDE.md defines for aspirational markers.
2. T2's allocation-win and fusion-correctness deliverables are fully real and pass review; marking the runtime-win aspirational preserves those.
3. T3 (Strassen-Winograd) is likely to pull the runtime comparison into scope naturally; splitting it out now avoids a T3-internal scope fight later.
4. The alternative (Option B, new story) adds tracking overhead for an aspirational target the project already has 3 of at the epic level (`[aspirational]` lines for GF(p), GF(2^m), GF(2) performance targets).

## 4. What the next session must do

1. **Ask the user** which of A/B/C in §3 to execute. Use `AskUserQuestion` with the three options above verbatim.
2. Execute the chosen option:
   - **A:** edit `7e6183bb` description via `jit issue update --description ...`, update `benches/field_matrix_fusion_results.md` with measured numbers, commit with scope `docs(jit:7e6183bb): ...`, re-run `code-review` gate. Expect PASS.
   - **B:** create new story via `jit issue create ...`, wire deps, update epic description's `## Child stories` table, close T2 with current gate state honest-marked, commit + run gates on T2.
   - **C:** `jit issue update 7e6183bb --state rejected --reason "runtime-win deferred to ad597ede"`, amend T3 description, do not touch commits.
3. After T2 is resolved, transition `d48a3cfd` story to `done` (requires T1 + T2 + T3, so T3 has to ship too).
4. Dispatch **T3 `ad597ede`** (Strassen-Winograd). Still touches `crates/gf2-core/src/field/matrix.rs` and should NOT be dispatched in parallel with anything else that touches `matrix.rs` or `expr.rs`.
5. Proceed per progress file's wave plan: `83b1ad8b` (trsm/trmm/trtri/trtrm), `c3f8c1cb` (PLE), `ae1d1e88` (inv/solve/det), `e47231cd` (charpoly tasks), `64c88ae4` (bench tasks).
6. Epic close.

## 5. Traps — do not repeat these

**Carried forward from session 1 (still binding):**

- PLE-first over LU. No `BitMatrix`/`FieldMatrix<GF(2)>` unification. Dispatch tasks (not oversize stories) for `d48a3cfd`/`64c88ae4`/`e47231cd`. `64c88ae4` runs in a pinned container only — never `apt install libfflas-ffpack-dev` on host. Per-story criterion benches are `[hard]`. Don't re-add the transitively-redundant epic→{8a90882e, ae1d1e88} direct edges. SIMD foundation is done (epic `e095a100`) — no new SIMD kernels inside the matrix layer. GPU epic `16283d6f` is out-of-scope for this epic.

**Carried forward from session 2 (still binding):**

- Reviewer drift is real; Tier 1.5 prior-findings regression check is mandatory. Workers silently defer via their own design docs; Tier 2.75 is mandatory. Rework prompts must be symmetric. Right-scalar `Mul<F>` bounded on `F: FiniteField`, left-scalar is per-`ConstField`-family (orphan rule). `with_capacity(rows, cols)` fills with zeros. Zero-inner-dim `gemm`/`matvec`/`matvec_transpose` panic on runtime-context fields with a locked message. `FiniteField::zero_hint()` is the `ConstField`/non-`ConstField` distinguisher. `Transposed<M>` is minimal. Design-doc "Follow-up work" bullets must cite issue Non-goals.

**Carried forward from session 3 (still binding):**

- `impl<F, E> From<E> for FieldMatrix<F> where E: Evaluate<F>` does NOT compile as a bare blanket — core's reflexive `impl<T> From<T> for T` overlaps. Session 4 adopted the **sealed `ProxyExpr` marker** pattern (not per-proxy From impls, not inherent `eval`) — this works cleanly and is documented in the `expr.rs` module header and in `dev/plans/expression_templates_design.md` §6.4 amendment.
- `jit_gate_pass` MCP return is misleading — always inspect `jit gate check-all` after.
- Serialize wave dispatches by default (cargo cache + parallel agent isolation).
- Rework budget baseline: 1 round per issue was the pre-session-4 norm; 7e6183bb broke that — plan for up to 3 rounds on implementation-heavy tasks with performance criteria.
- Worker design docs can contradict their own parity tables; lead must polish.

**New from session 4 — must not be repeated:**

- **Don't write "fused must be measurably faster" as `[hard]` at a specific n without an order-of-magnitude analysis of where the win comes from.** T2's criterion implicitly assumed that saved allocation at n=1024 would dominate, but the allocation is ~10⁻³ of compute. Future performance criteria on this epic must either (a) state an aspirational number with "if measured, aspire to X%", or (b) pick an n where the target is physically achievable (e.g., small n where allocation/one-off-setup dominates).
- **Don't expose "either rewrite kernel OR amend doc" options to a worker in a rework prompt.** Session-4 R1 and R2 rework prompts did this once each (offering 3-alternative fix routes in Fix A's A/B/C style). This leaks scope decisions to the worker, who may pick the cheapest option (doc change instead of real code fix). Keep worker prompts strictly about code changes. Scope decisions go to `AskUserQuestion`. R3 rework did this correctly (one prescribed fix, user scope decision made first).
- **MCP `jit_gate_pass` has a 10-minute tool-level timeout.** `ai-review.sh` for a 2260-line module takes 7-12 min. Use `Bash(jit gate pass <issue> code-review, run_in_background=true, timeout=900000)` instead and poll `.jit/gate-runs/` for the new folder. MCP `jit_gate_pass` that times out leaves NO gate-run recorded (the subprocess is killed).
- **AI reviewer may run its own local bench** to verify `[hard]` performance claims. Worker's self-reported numbers can be inverted by the reviewer's rerun under different Criterion settings; always cross-check with a longer-sample run before signing off. Session 4 caught this: worker reported "fused 6.5% faster"; reviewer reported "eager 6.6% faster"; lead's 20-sample rerun sided with reviewer.
- **Commit `e94eb23` is preserved intentionally.** It is a strict improvement over R2's naive 3-loop fused kernels and should NOT be reverted when the user picks a scope option. If the user picks Option A (`[aspirational]`), the bench md gets updated with the 3.227 s fused / 3.163 s eager numbers from this commit's bench. If Option B or C, the commit is useful baseline for whoever picks up the runtime-win work.

## 6. Reference artefacts

- Epic: `jit issue show bb85c68a`
- Progress file: `dev/active/bb85c68a-progress.json`
- Prior handoffs: `bb85c68a-handoff.md`, `bb85c68a-handoff-2.md`, `bb85c68a-handoff-3.md`
- Session-4 commits (all on `main`):
  - `bf78239` — T2 initial (proxy types + fusions + sealed `ProxyExpr` bridge)
  - `a2c5cbb` — R1 rework (αAᵀ·B+βC fusion + serial trace tests + alloc evidence + panic tests + doc fix)
  - `c267f25` — R2 lead polish (`kernel_counts` doc + bench md doc nits)
  - `e94eb23` — R3 kernel rewrite (blocked `gemm_with_beta_concrete` / `gemm_trans_a_concrete` / `gemm_trans_a_with_beta_concrete`)
- Gate runs of interest:
  - `1a004c00` — R1 code-review FAIL (F1-F5 reviewer findings)
  - `eb66ad99` — R2 code-review FAIL (G1-G2 doc overclaims)
  - `9cdd2519` — R3 code-review FAIL (H1 empirical fused-slower-than-eager finding)
- Design docs (unchanged since session 3 except the §6.4 / §6.5 amendment landed in `a2c5cbb`):
  - `dev/plans/fflas_ffpack_analysis.md`
  - `dev/plans/dumas_pernet_takeaways.md`
  - `dev/plans/armadillo_ux_mapping.md`
  - `dev/plans/expression_templates_design.md` — §5.4 + §6.4 + §6.5 amended for ScaledTransposedProduct and the sealed ProxyExpr bridge.
  - `dev/active/ab791e27-design.md`
  - `dev/plans/gpu_fieldmatrix_sketch.md` (follow-on epic `16283d6f`; out-of-scope)
- Bench measurements to remember for Option A:
  - n=1024 Mersenne-31: fused 3.227 s, eager 3.163 s (20-sample Criterion, sample size + measurement time 10 s)
  - n=256 Mersenne-31: ~50 ms both (well inside Criterion noise threshold)

# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 6

**Date:** 2026-05-25
**Session number:** 6
**Prior handoffs:** session 1, 2, 3, 4, 5 (`dev/active/026fc832-handoff*.md`)

## Current state

- Epic: `026fc832` — state: `backlog` (will stay so until wave 6.5+7+8 land; assignee `agent:project-lead`)
- Wave in progress: **wave 6.5 — 40195c09 + 74ba1cdc DISPATCHED (background)**; not yet complete
- Children summary: 16 done (615db3b9, 27bb2f75, 5ce13bae, aaa847cf, 52cce970, a70b1c70, bd9c6e13, 68cdf4c8, 91429c1c, fc182ed5, 41096af5, 873cbec1, e8a0c47a, 2e8c5a29, 24a93e4e, feb15da9), 3 in_progress (b0fa00af scorecard v1 published pending v2; 98336ab4 awaiting 74ba1cdc; 40195c09 + 74ba1cdc just dispatched), 4 backlog (6823c8a0, 869ce43b, 8df0c501, 6613abf4 — wave 7)
- Active claims: epic + b0fa00af + 98336ab4 + 40195c09 + 74ba1cdc (all `agent:claude`)
- Open escalations: **session-6 escalations RESOLVED** — user picked Option A (lift `gemm_axpy_into_view`) for the dispatch-architecture question, and Option B (do GEMM-engineer task now) for 98336ab4 SHORTFALL cells. 2 new tasks filed + DAG wired.
- Progress file: `dev/active/026fc832-progress.json` (will be updated this session; reflects wave 6/6.5/7a/7b/8 plan)

## What just happened

Wave 6 completed; new architectural prerequisite uncovered and addressed.

- **Wave 6 dispatched (4 parallel workers, sonnet):** `2e8c5a29` (PLE design), `24a93e4e` (echelon design), `feb15da9` (invert design), `98336ab4` (n=4096 fgemm benches). All 4 produced output.
- **Wave 6 R0 review:** `2e8c5a29` FAIL (bench table incomplete + GF(65521) scope), `24a93e4e` FAIL (GEMM operand shape contradiction), `feb15da9` FAIL (non-existent `F::SMALL_PRIME_SIMD_ELIGIBLE`).
- **Wave 6 R1 rework dispatch (sonnet):** R1 directive (from lead) was to "route through `gemm_axpy_into_view`" — turned out factually wrong.
- **Wave 6 R1 review:** `2e8c5a29` PASS; `24a93e4e` FAIL + `feb15da9` FAIL both citing "gemm_axpy_into_view doesn't have small-prime fast path".
- **Escalated to user** (lead-side fact error): `gemm_axpy_into_view`'s per-cell `dot_product_slices` only auto-dispatches medium-prime u16; small-prime whole-GEMM is via `gemm()` only. User decision: (Q1) lift `gemm_axpy_into_view`; (Q2) GEMM-engineer Route A + fp_medium now.
- **Filed 2 new tasks** (commit `38beb7c2`): `40195c09` (lift) + `74ba1cdc` (GEMM-engineer, label `ppc-kernel:C2`). Wired DAG: 6823c8a0→40195c09 (transitive: 869ce43b, 8df0c501, 6613abf4 reach it); 98336ab4→74ba1cdc.
- **Wave 6 R2 reworks dispatched (sonnet):** updated designs to cite 40195c09 as prerequisite. `24a93e4e` R2 PASS; `feb15da9` R2 FAIL (medium-prime helper chain still wrong + transitive-dep wording).
- **feb15da9 R3 LEAD-DIRECT** (escalation policy 5a — same root-cause repeated 3 times): lead grep-verified the actual chain (`dot_product_slices → F::try_fp_simd_dot_product → fp_medium_try_dot_product`), edited 2 sections, committed `3d6fdaa2`. R3 PASS.
- **Wave 6 close (commit `c924890e`):** all 3 designs gate-passed (code-review + cargo-ci + doc-review) and marked done.
- **Wave 6.5 dispatch (this session, parallel worktrees, opus):** `40195c09` (lift) at `worktree-agent-40195c09`, `74ba1cdc` (GEMM-engineer) at `worktree-agent-74ba1cdc`. Both anchored at `c924890e`. Both running in background as of session end.
- **98336ab4**: worker reported done with 4 PASS + 2 SHORTFALL cells (GF(251)/4096 ratio 0.536 + GF(65521)/4096 ratio 0.521), GF(7)/256 -37.3% non-regression delta (instruction-cache warmup methodology mismatch). **Worker branch NOT merged** — 98336ab4's bench infra will be brought to main alongside the 74ba1cdc re-bench at close time.

## What to do next

In order of priority:

- [ ] **Monitor wave 6.5 workers** (`40195c09` + `74ba1cdc`). When each completes, run the 6-tier review per `references/lead-review-protocol.md`. `40195c09` is smaller (1-2 hr scale); `74ba1cdc` is multi-day perf engineering and may not complete in one shot.
- [ ] **Merge 40195c09 first** (when it lands) — it's the prereq for everything else. Run code-review + cargo-ci + doc-review gates; close if PASS.
- [ ] **Resolve 74ba1cdc** when it lands — may be PASS, partial PASS (hit a wall), or escalate. Per session-6 Q2 user choice, the SC requires both GF(251)/4096 + GF(65521)/4096 ≤ 1.5× ratio.
- [ ] **Merge 98336ab4** after 74ba1cdc closes. Re-bench all 6 n=4096 cells with warmup-matched protocol (include n=64 in bench filter to avoid the GF(7)/256 -37.3% icache trap). The 98336ab4 worker branch at `worktree-agent-98336ab4` (commit `3dca7bf7`) has the bench infra + proptests; it can be cherry-picked or merged after rebasing onto post-74ba1cdc main. Then 98336ab4 closes.
- [ ] **Wave 7a dispatch**: `6823c8a0` (panelized PLE impl) — opus, on main (single foundational impl). Now depends on 40195c09. Worker reads `dev/active/2e8c5a29-panelized-ple-design.md`.
- [ ] **Wave 7b dispatch** (parallel worktrees after PLE lands): `869ce43b` (echelon impl, reads 24a93e4e + 6823c8a0), `8df0c501` (invert impl, reads feb15da9 + 6823c8a0), `6613abf4` (solve impl, reads 6823c8a0). All opus. SERIALIZE bench runs across these 3 (one at a time on CCX1 cores 6-11) but parallel file edits via worktrees.
- [ ] **Wave 8**: re-dispatch `b0fa00af` with a "v2 scorecard" prompt that updates § 4 + § 8 with the Phase 6 closures + post-74ba1cdc n=4096 cells. Verifies SC#5 of `026fc832` (zero unfixed FAIL cells).
- [ ] **Epic close (Section 10)** after `b0fa00af` v2 lands.

## Traps — do not repeat these

**Carry forward** (link, don't copy): session 1, 2, 3, 4, 5 handoffs' Traps sections. All still in force.

**New session-6 traps:**

- **`gemm_axpy_into_view` does NOT route the small-prime SIMD fast path today** (pre-40195c09). It does per-cell `dot_product_slices` (matrix.rs:2898-2916) which calls `F::try_fp_simd_dot_product` (vec.rs:480-492). For `Fp<P>`, that maps to `fp_medium_try_dot_product` (gfp/mod.rs:654-661, simd_ops.rs:1388) — medium-prime u16 only. The small-prime whole-GEMM (`fp_small_try_gemm_classical` at simd_ops.rs:654) is reached ONLY via `gemm()` (matrix.rs:2642). DO NOT claim in any future design that "trsm_* via gemm_axpy_into_view inherits small-prime speedup" without citing 40195c09 as a prerequisite. R0/R1/R2 of feb15da9 all stumbled on different mis-statements of this dispatch chain.

- **`gemm()` uses DIFFERENT medium-prime helpers than `gemm_axpy_into_view`.** `gemm()` (matrix.rs:2642) packs once into u16 buffers via `F::try_pack_fp_medium_u16` (gfp/mod.rs:665 → simd_ops.rs:1443) then dots via `F::try_fp_simd_dot_packed_u16` (gfp/mod.rs:671 → simd_ops.rs:1470 / `fp_medium_try_dot_packed`). `gemm_axpy_into_view` does per-cell `dot_product_slices → F::try_fp_simd_dot_product → fp_medium_try_dot_product` (simd_ops.rs:1388). Different helpers. The packed-u16 amortization is a `gemm()`-only optimisation. Don't conflate the two paths.

- **Lead-side dispatch-prompt facts must be grep-verified.** Three rework rounds were lost to lead-side claims about the dispatch graph that were "approximately right" but factually wrong in detail. Per `feedback_dispatch_prompt_facts`, grep the codebase BEFORE writing any "the dispatch goes from X → Y → Z" claim. R3 lead-direct on feb15da9 worked because the lead grep-verified each cited file:line before editing.

- **Escalation policy 5a — same root-cause repeated 3 times = escalate.** feb15da9 had 3 review rounds all citing some variant of "helper-chain mismatch through the dispatch graph". This triggers 5a even before MAX_REWORK_ATTEMPTS=2 is hit. Don't dispatch a 4th worker on the same root cause; either lead-direct fix (with grep-verified facts) or escalate to user.

- **`jit gate pass cargo-ci` for design-only changes**: even when the design adds no code, the gate run is needed to record PASS state on the issue. Run it in addition to code-review + doc-review. Trivial to satisfy (cargo cache hot), but JIT requires the per-issue record.

- **`jit doc add` write-through still applies in this session.** Same trap as session 5: workers running `jit doc add` from their worktree modify main's `.jit/issues/<id>.json` (MCP writes through to canonical .jit). When merging worker branches, the worker's commit may or may not include the `.jit` change. Verify with `jit doc list <id>` AFTER merge, and re-`jit doc add` from main if the list shows 0 documents.

- **The `[aspirational]` marker is STILL not a free pass.** Session 5's trap remains. Session 6 reinforced: when 98336ab4 had 2 SHORTFALL cells, the worker self-attributed them to "structural [aspirational]" status. Lead escalated to user instead of applying. User chose engineering work (74ba1cdc) over an [aspirational] amendment.

- **`fflas's reference numbers are SINGLE-THREAD.** Per `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md:24`: "All container measurements are single-threaded." DO NOT speculate that fflas's 159 Gop/s number includes OpenBLAS threading — it doesn't. The per-core gap (Route A 36% of peak vs OpenBLAS 70%) is algorithmic, not threading-related. Closing it requires GEMM engineering (BLIS-style blocking, register tiling, persistent panel packing), which is what 74ba1cdc does.

- **DO NOT introduce a BLAS dependency.** User directive (session 6 Q1+Q2): no BLAS in gf2. Even though OpenBLAS's sgemm closes the n=4096 gap for fflas, the gf2 path must stay pure Rust + AVX2. AVX-512 is also out of scope (epic 7f809931).

- **Wave 6.5 workers (40195c09 + 74ba1cdc) run in parallel on the same host.** CCX1-pinned benches (`taskset -c 6-11`) must serialize. The dispatch prompts told both workers about this. If the next lead resumes mid-wave-6.5, monitor for bench contention.

## Open questions needing user input

None unresolved. The session-6 escalations (Q1 dispatch architecture + Q2 98336ab4 SHORTFALL) were resolved with the user's choices. Wave 6.5 dispatch is consistent with those choices.

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (to be updated; reflects wave 6/6.5/7a/7b/8 plan)
- Session 1-5 handoffs: `dev/active/026fc832-handoff*.md` — all trap sections still in force
- **Wave 6 design closures** (all done):
  - `dev/active/2e8c5a29-panelized-ple-design.md` (panelized PLE algorithm; 377→411 lines after R1)
  - `dev/active/24a93e4e-blocked-echelon-design.md` (blocked echelon; 507→approx 660 lines after R1+R2)
  - `dev/active/feb15da9-blocked-invert-design.md` (blocked invert via PLE; 448→approx 540 lines after R1+R2+R3 lead-direct)
- **Wave 6.5 tasks** (in_progress, dispatched session 6):
  - `jit issue show 40195c09` — lift `gemm_axpy_into_view`; worktree `worktree-agent-40195c09` at `c924890e`
  - `jit issue show 74ba1cdc` — GEMM-engineer Route A + fp_medium at n=4096; worktree `worktree-agent-74ba1cdc` at `c924890e`
- **Wave 7 implementation tasks** (backlog, blocked):
  - `jit issue show 6823c8a0` — panelized PLE impl (blocked on 40195c09)
  - `jit issue show 869ce43b` — echelon impl (blocked on 24a93e4e + 6823c8a0; 40195c09 transitive via 6823c8a0)
  - `jit issue show 8df0c501` — invert impl (blocked on feb15da9 + 6823c8a0)
  - `jit issue show 6613abf4` — solve impl (blocked on 6823c8a0)
- **98336ab4 worker branch** (in_progress, blocked on 74ba1cdc):
  - `worktree-agent-98336ab4` at `3dca7bf7` (base `8142de7c`; needs rebase before merge)
  - Evidence doc: `dev/bench_results/2026-05-25-98336ab4-fgemm-n4096.md` (4 PASS + 2 SHORTFALL; GF(7)/256 -37.3% from icache warmup)
- **b0fa00af scorecard v1**: `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` (kept open pending v2 republish after wave 8)
- **615db3b9 plan**: `dev/active/615db3b9-finite-field-la-sota-plan.md` (Phases 1-5; Phase 6 panelized push is new this epic, not in the plan doc)
- **Worktree dispatch protocol**: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- **Reference host**: AMD Ryzen 9 5900X (Zen 3), 12c/24t, AVX2+BMI2+VAES+VPCLMULQDQ, no AVX-512
- **fflas reference numbers**: `dev/bench_results/2026-04-26-reference.csv` + supplements; verified single-thread per a70b1c70-phase0-controls.md:24

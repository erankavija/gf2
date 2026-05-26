# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 7

**Date:** 2026-05-26
**Session number:** 7
**Prior handoffs:** session 1, 2, 3, 4, 5, 6 (`dev/active/026fc832-handoff*.md`)

## Current state

- Epic: `026fc832` — state: `backlog` (assignee `agent:project-lead`)
- Wave in progress: **wave 6.5 — 40195c09 R1 work done on main; code-review gate retry blocked by MCP timeout. 74ba1cdc serial re-dispatch pending (user-approved with flock guard).**
- Children summary: 16 done, 3 in_progress (b0fa00af v1 pending v2; 98336ab4 awaiting 74ba1cdc; 40195c09 R1 done on main, gate retry pending; 74ba1cdc wall-hit then user-approved serial re-dispatch), 4 backlog (6823c8a0, 869ce43b, 8df0c501, 6613abf4 — all blocked on 40195c09 closing).
- Active claims: epic + b0fa00af + 98336ab4 + 40195c09 (currently agent:claude) + 74ba1cdc (agent:claude).
- Open escalations: **none unresolved**. Session-6 Q1+Q2 + Q3 (74ba1cdc wall-hit) all answered.
- Progress file: `dev/active/026fc832-progress.json` (still session-6 schema; needs session-7 update by next lead).

## What just happened

Wave 6.5 dispatched parallel; 40195c09 PASS locally but FAIL code-review on 2 surgical SC gaps; 74ba1cdc hit a wall (lead-dispatch error + architectural depth). R1 rework landed for 40195c09; gate retry blocked by tool timeout.

- **Wave 6.5 parallel dispatch (lead error):** dispatched `40195c09` (lift) and `74ba1cdc` (GEMM-engineer) simultaneously in worktrees, violating the CCX1-serialization warning in both prompts. Caused host contention for `74ba1cdc`'s benches.
- **`40195c09` R0 completed PASS:** worker chose Strategy 1 (scratch-buffer + add-into-view). 6 trsm cells show 1.6%-4.9% speedup; non-regression max +0.1%. Commit `6380f760` merged to main.
- **`40195c09` R0 code-review FAIL** (run_id `4c6459db`): 2 surgical findings — (1) proptest only covers `Fp<251>`, SC requires all 6 primes; (2) PLE Schur-update non-regression missing from evidence doc.
- **`74ba1cdc` wall-hit:** worker correctly stopped at >5% baseline divergence (caused by lead error parallel-dispatch + host contention). 344-line evidence doc committed (`193e0125 → 558a208f` after rebase). Worker recommended Phase 6a/6b decomposition.
- **Escalation (Q3 — 74ba1cdc wall-hit):** user picked Option B (re-dispatch same scope, serially, with flock-based CCX1 mutex). NOT Option A (decomposition). NOT Option C (aspirational amendment).
- **`40195c09` R1 rework dispatch (sonnet, solo on host):** addressed both R0 findings. Commit `2a8b65a5` merged to main.
  - 5 new proptests `prop_gemm_axpy_into_view_fp{7,31,127,241,65521}_matches_oracle` (in addition to existing `fp251`). All 9 proptests pass.
  - PLE Schur-update non-regression measured at n=256 via new `examples/ple_timing.rs` (the existing `fieldmatrix_ple` bench hangs on rank-deficient n=4096 init): GF(7) +0.3%, GF(251) 0%, GF(65521) -0.2%. All within ±5%.
  - All local gates pass: fmt clean, clippy clean, nextest 3863/3863 passed.
- **`40195c09` code-review gate retry FAILED to record** (MCP timeout at 10 min on the larger diff). The R1 commit is on main but the gate state still shows the R0 failure. **NEXT LEAD: retry `jit gate pass 40195c09 code-review`** — may need to invoke the underlying `./scripts/ai-review.sh` directly with JIT_CONTEXT_FILE set, or wait/retry until the MCP completes within budget.

## What to do next

In order of priority:

- [ ] **Retry `jit gate pass 40195c09 code-review`.** Two timeouts already at 10-min MCP limit; the underlying AI reviewer takes ~5-7 min normally on this codebase but the larger R1 diff (with PLE proptest + bench logs) appears to push past the MCP tool's limit. Options:
  - Retry the MCP call once more (might catch a cache-warm path).
  - If still timing out: invoke `./scripts/ai-review.sh 40195c09` with `JIT_CONTEXT_FILE` set manually (the script needs context that's normally provided by JIT).
  - If still failing: file a follow-up `jit` infrastructure bug or surface to the user — the auto code-review on large diffs is not currently completing.
- [ ] **Pass `40195c09` cargo-ci + doc-review gates** after code-review unblocks. Mark 40195c09 done. JIT state commit.
- [ ] **Clean up 40195c09 R1 worktree:** `git worktree remove .claude/worktrees/agent-40195c09-r1 --force && git branch -D worktree-agent-40195c09-r1`.
- [ ] **Re-dispatch 74ba1cdc with flock guard** (per user Q3 directive). Solo on host (40195c09 closed; no other workers). Same scope (BLIS MC/KC restructure for Route A + panelized fp_medium kernel). Worker now has the prior wall-hit evidence doc (`dev/bench_results/2026-05-25-74ba1cdc-fgemm-engineering.md`) which contains the full bottleneck analysis + design sketch — feed this into the dispatch prompt as required reading.
  - Add a small wrapper script `dev/benchmarks/ccx1-bench-flock.sh` (or instruct inline) that wraps bench commands with `flock -x /tmp/gf2-ccx1.lock taskset -c 6-11 ...` to ensure CCX1 serialization. The lock file path can be any agreed convention.
  - Worker may again wall on architectural complexity even with quiet host. If so, escalate to user with the second wall-hit evidence + reconsider Option A (Phase 6a/6b decomposition).
- [ ] **After 74ba1cdc closes**, **re-bench 98336ab4 cells with warmup-matched protocol** + new kernels. Worker (lead-direct or sonnet) reads `dev/bench_results/2026-05-25-98336ab4-fgemm-n4096.md` + the 74ba1cdc evidence doc; re-runs 6 cells at n=4096 (GF(7/31/127/241/251/65521)) with n=64 prefix in bench filter for icache warmup. Update evidence + close 98336ab4.
- [ ] **Wave 7a dispatch:** `6823c8a0` (panelized PLE impl) — opus, on main. Reads `dev/active/2e8c5a29-panelized-ple-design.md` + assumes 40195c09 lift landed.
- [ ] **Wave 7b dispatch** (parallel worktrees after PLE lands): `869ce43b` (echelon), `8df0c501` (invert), `6613abf4` (solve). All opus. SERIALIZE bench runs across these 3 (flock guard).
- [ ] **Wave 8:** re-dispatch `b0fa00af` with v2 scorecard prompt covering all Phase 6 closures.
- [ ] **Epic close (Section 10)** after b0fa00af v2 lands.
- [ ] **Update `dev/active/026fc832-progress.json`** to reflect session-7 state (current_wave=6.5; add 40195c09 R1 entry; add 74ba1cdc wall-hit + re-dispatch plan; add session-7 to session_history).

## Traps — do not repeat these

**Carry forward** (link, don't copy): session 1–6 handoffs' Traps sections. All still in force.

**New session-7 traps:**

- **Parallel-dispatch CCX1 violation cost a wall-hit.** I (session-7 lead) dispatched `40195c09` + `74ba1cdc` in parallel worktrees despite both prompts containing the "only one bench at a time on CCX1" warning. The result: `74ba1cdc` got host-contaminated baselines (10% divergence), triggering the dispatch protocol's STOP-at->5% rule. **Future leads: dispatch CCX1-bench tasks SERIALLY. The flock guard the user approved is the mechanical fix; until that script lands, manual serialization is mandatory.**

- **The `jit gate pass code-review` MCP call has a 10-min effective timeout that the AI reviewer can exceed on large diffs.** `40195c09` R0 took 316s (passed budget). R1 retries timed out 2× at 10 min without recording a result. The MCP server's `ai-review.sh` script needs `JIT_CONTEXT_FILE` set externally; invoking it directly without that env var fails immediately with "JIT_CONTEXT_FILE not set." Path forward: either retry until success, find a way to extend the MCP timeout, or file a JIT infrastructure bug. Don't assume the gate has run if the MCP call timed out — always check `jit gate check-all <id>` to see the actual recorded state.

- **The `fieldmatrix_ple` Criterion bench hangs at startup on rank-deficient n=4096 init** (per 40195c09 R1 worker discovery). It calls `gemm` on 4096×2048×4096 during fixture build. For PLE-specific bench needs at small n, use a lightweight `examples/ple_timing.rs` pattern (the R1 worker added this — see `crates/gf2-core/examples/ple_timing.rs`). Don't try to use the Criterion bench for n>=1024 PLE work without first dropping the deficient regime from the fixture.

- **`F::try_pack_fp_medium_u16` + `try_fp_simd_dot_packed_u16` are `gemm()`-path helpers, NOT used by `gemm_axpy_into_view`** (carried forward from session-6 trap). The actual medium-prime path through `gemm_axpy_into_view → dot_product_slices → F::try_fp_simd_dot_product → fp_medium_try_dot_product` is at `simd_ops.rs:1388`. After `40195c09` lands, both small-prime (lifted) and medium-prime (was already there) paths are reached, but via DIFFERENT internal helpers.

- **R0 reviewer's nested "delegated review" + "verifier review" doubles the runtime.** The R0 review ran a "delegated" reviewer that PASSed (5 min), then a "verifier" reviewer that FAILed with the 2 surgical findings (also ~5 min). Total ~316s. R1 + retries timed out at 600s because the verifier's stricter standard takes longer on larger diffs. Future-lead note: budget for `~5-10 min` per code-review gate run on substantial diffs; consider preemptively splitting commits into smaller reviewable chunks.

## Open questions needing user input

None unresolved. The session-6 + session-7 escalations were all answered. The session-7 retry-blocker (gate timeout) is an infrastructure issue, not a user-decision issue — the next lead retries the gate.

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (needs session-7 update; still session-6 schema)
- Session 1-6 handoffs: `dev/active/026fc832-handoff*.md` — all trap sections still in force
- **Wave 6 design closures** (all done):
  - `dev/active/2e8c5a29-panelized-ple-design.md`
  - `dev/active/24a93e4e-blocked-echelon-design.md`
  - `dev/active/feb15da9-blocked-invert-design.md`
- **Wave 6.5 status:**
  - `40195c09`: R0 + R1 commits on main (`6380f760` + `2a8b65a5`). All 9 proptests pass, PLE non-regression measured. **Gate retry blocked by MCP timeout.** Evidence doc: `dev/bench_results/2026-05-26-40195c09-gemm-axpy-lift.md`. R1 worker added `crates/gf2-core/examples/ple_timing.rs` as a lightweight PLE bench harness.
  - `74ba1cdc`: wall-hit evidence at `dev/bench_results/2026-05-25-74ba1cdc-fgemm-engineering.md` (344 lines, has BLIS MC/KC design sketch + Phase 6a/6b decomposition recommendation). Worker branch deleted; **user picked re-dispatch (not decomposition)**.
- **Wave 7 implementation tasks** (backlog, blocked on 40195c09):
  - `jit issue show 6823c8a0`, `869ce43b`, `8df0c501`, `6613abf4`
- **98336ab4 worker branch** (in_progress, blocked on 74ba1cdc): `worktree-agent-98336ab4` at `3dca7bf7` (base `8142de7c`, stale; needs rebase). Evidence doc: `dev/bench_results/2026-05-25-98336ab4-fgemm-n4096.md` (4 PASS + 2 SHORTFALL + GF(7)/256 -37.3% icache delta).
- **b0fa00af scorecard v1**: `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` (kept open pending v2 republish after wave 7+8 + 98336ab4 closure).
- **Worktree dispatch protocol**: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`. **A flock-based CCX1 mutex extension is now needed for parallel bench workers — open follow-up.**
- **Reference host**: AMD Ryzen 9 5900X (Zen 3), 12c/24t, AVX2+BMI2+VAES+VPCLMULQDQ, no AVX-512.
- **fflas reference numbers**: verified single-thread per `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md:24`.

## Active worktrees + branches at session-7 end

```
/home/vkaskivuo/Projects/gf2                                     43be2723 [main]
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-40195c09-r1 2a8b65a5 [worktree-agent-40195c09-r1] (R1 merged; safe to remove)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-98336ab4    3dca7bf7 [worktree-agent-98336ab4] (waiting on 74ba1cdc)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-9480f8a6    53d2b40f [worktree-agent-9480f8a6] (unrelated; preserved)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-30e98ef1-d6 4440b228 [worktree-agent-30e98ef1-d6] (unrelated; preserved)
```

Main at the time of handoff: HEAD `2a8b65a5` (per the merge) but the in-flight session-7 work hasn't bumped past that for non-trivial reasons (only `chore(jit:40195c09): record code-review R0 FAIL` at `43be2723` is between main HEAD and prior merges — see `git log --oneline` for the full chain).

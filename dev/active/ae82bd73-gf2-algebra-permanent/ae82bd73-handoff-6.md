# Handoff — Fast matrix permanents over F_3 / F_5 / F_7 via packed bipedal arithmetic (ae82bd73) — session 4 final

**Date:** 2026-05-11 (session 4 close, after marathon W2 sub-wave 2c)
**Session number:** 4 (this is the second handoff for session 4; supersedes `handoff-5.md` which was written mid-session)
**Prior handoffs:** `ae82bd73-handoff-1.md`, `ae82bd73-handoff-2.md`, `ae82bd73-handoff-3.md`, `ae82bd73-handoff-4.md`, `ae82bd73-handoff-5.md` (read all in order; traps from each carry forward unless explicitly resolved)

## Current state

- Epic: `ae82bd73` — state: `backlog`.
- **W2 fully closed.** All 6 issues (T7, T8, T9, T10, T11, Sa) done.
- W3 (SIMD + multi-word + parallel) ready to dispatch — 4 implementation issues + 3 simulation issues.
- Active claims: none (T10 + T11 + Sa all auto-released on `--state done`).
- Open escalations: none.
- Progress file: `dev/active/ae82bd73-progress.json` (updated this handoff).
- Worktrees: `worktree-agent-b315564a` + `worktree-agent-1cd3eb09` exist but their work is merged. Remove with `git worktree remove .claude/worktrees/agent-b315564a && git worktree remove .claude/worktrees/agent-1cd3eb09` before W3 dispatch.

## What just happened (session 4, complete narrative)

This was the longest session of the epic. Started with T9 in flight (3 unresolved review findings from session 3), ended with all of W2 closed.

### Issues closed in close-order

1. **T9 (`b0857ae9` — permanent_bipedal3 single-u64 fast path)** — 1 worker rework cycle + 2 lead-direct fixes.
   - User-approved amendments: criterion 1 (n ≤ 63), criterion 3 (oracle = permanent_mod3_reference for n ∈ {20, 24}).
   - Sonnet worker added `Bipedal3::fold_mul_first_n` and refactored permanent_bipedal3 to route through Bipedal3 SSOT API (eliminating inline raw-u64 paper §2.2 ops).
   - Lead-direct: cross-issue SSOT consolidation of three `random_matrix` helpers into `crate::testutil` (commit `fcbaa003`).

2. **T10 (`b315564a` — Criterion bench skeleton)** — 3 worker rework cycles + 3 lead-direct fixes + 3 user-approved criterion amendments.
   - 2026-05-11: split sweep ranges (was n=8..36 for both, now ref=8..24 and bipedal=8..36).
   - 2026-05-11b: Lcg-via-testutil (was StdRng); drop ref n=24 (overshoots 60s/cell).
   - 2026-05-11c: trim bipedal sweep n=8..28 (was 8..36; n=32 measured at 9.4s/iter → 94s/cell, n=36 at 150s/iter → 1495s/cell).
   - Lead-direct: lift `testutil` from `#[cfg(test)] pub(crate)` to `pub mod` under `#[cfg(any(test, feature = "test-support"))]` (matching gf2-core workspace pattern); add `test-support` feature + self-dev-dep.
   - Lead-direct: lower `measurement_time` from 45s to 25s (forces Criterion linear-mode to pick d=1 at n=28, keeping total under 60s/cell).

3. **T11 (`1cd3eb09` — Test-vector suite)** — 2 worker rework cycles + 2 lead-direct fixes + 1 user-approved amendment.
   - 2026-05-11: align criteria 3/4/6 with T9 amendments (default-tier n=4..12, slow-tier n=16/20/24 with `#[ignore = "sim: ..."]`; oracle = permanent_mod3_reference for n ∈ {20, 24}).
   - Lead-direct: same test-support refactor as T10; deletion of redundant `to_bipedal` wrapper around `Bipedal3Matrix::from_row_major`.

4. **Sa (`96dcbec4` — paper Table 2 slope reproduction)** — 3 worker rework cycles + 4 lead-direct fixes + 2 user-approved amendments.
   - Sonnet worker initial pass: example at `crates/gf2-algebra/examples/paper_repro_slope.rs` + writeup at `dev/plans/sa_paper_repro_slope.md`.
   - Lead-direct fixes: added SHA-256 `input_hash` column (sha2 dev-dep), dynamic date computation (inline Hinnant civil-from-days, no chrono dep), Bessel-corrected sample variance, per-sample inner-iter calibration for stable timing at small n, added 3 unit tests for helpers.
   - 2026-05-11: criterion 1 (sweep n=8..24 instead of paper's n=24..36) + criterion 5 (reproducibility = input determinism, not bit-identical CSV).
   - 2026-05-11b: criterion 2 (reference value = ln(2) + mean(1/n) over actual sweep, not paper's asymptotic 0.693). Observed slope 0.7634 vs range-adjusted reference 0.7636 → residual ratio 0.9997.

### User escalations resolved this session (7 total)

1. T9 criterion 1 amendment (n ≤ 63).
2. T9 criterion 3 + epic criterion 2 amendment (oracle = permanent_mod3_reference for n ∈ {20, 24}).
3. T11 criterion 3/4/6 amendment (tier split + oracle).
4. T10 criterion 2 amendment (split sweep ranges; first cut).
5. T10 criterion 3 amendment (Lcg via testutil, not StdRng) — required two AskUserQuestion rounds because lead's initial framing didn't surface the actual trade-offs.
6. T10 criterion 2 amendment 2026-05-11c (bipedal sweep trim n=32 + n=36 after live measurement).
7. Sa criterion 1, 2, 5 amendments.

### Lead-direct cycles applied this session (12 total)

- T9 close-out: 2 (Bipedal3 SSOT refactor + random_matrix consolidation).
- T10: 3 (test-support feature + bench measurement_time tuning + bipedal trim).
- T11: 2 (test-support feature + to_bipedal removal).
- Sa: 4 (SHA-256 + dynamic date + sample variance + test-anchor fix).
- Workspace: 1 (epic-level + per-issue JIT description amendments).

### What this session taught — process improvements to absorb

1. **Workspace test-support pattern.** gf2-core uses `test-support = []` + self-dev-dep. Dispatching benches/integration tests without this is what caused the T10 + T11 SSOT-duplication review cycle. Memory candidate: `feedback_check_workspace_test_support_pattern`.

2. **Bench-target reviews run cargo bench live.** ai-review.sh's reviewer agent decides to validate criterion-4 wall-clock contracts by actually running `cargo bench`. Each cell can take minutes; total review wall-clock is 10-20 min for bench-target issues vs 3 min for test-only issues. Plan accordingly. Memory candidate: `feedback_bench_target_review_budget`.

3. **Criterion linear sampling.** Criterion at `sample_size(10)` runs `iters = [d, 2d, ..., 10d]` = 55·d iterations, not 10. Total time ≈ measurement_time when d > 1. To force d=1 (predictable timing), set measurement_time low enough that 55·1·mean_iter slightly overshoots it. This is the key knob for keeping bench cells under wall-clock budget. Memory candidate: `feedback_criterion_linear_sampling`.

4. **Slope criteria are range-dependent for O(n·k^n) algorithms.** When comparing observed slope to a source's published slope, account for the polynomial-correction term: integrated slope over a range = ln(k) + mean(polynomial_derivative). The paper's 0.693 was asymptotic; our 0.76 at n=8..24 was correct for OUR range. Memory candidate: `feedback_slope_reference_range_adjusted`.

5. **Reviewer extraction quirk.** ai-review.sh greps for `VERDICT:` literal. If the reviewer agent emits `RECOMMENDATION:` instead (as it did on T10 once), the gate fails extraction even with a PASS analysis. The user instructed NOT to touch the prompt to fix this; just budget for occasional re-runs.

### Process metric: session 4 in numbers

- Issues closed: 4 (T9, T10, T11, Sa).
- Worker rework cycles: 9.
- Lead-direct fixes: 12.
- User-approved criterion amendments: 7.
- AskUserQuestion rounds: 5.
- ai-review.sh gate-runs (gpt-5.4): ~14. Median ~5 min; T10 spikes to 12-20 min when reviewer runs live bench.
- Wall-clock: ~14 hours (across multiple wakeup cycles).

## What to do next

**Immediate (session 5 start):**

1. **Clean up the two stale worktrees** for T10 and T11 — both branches are merged into main:
   ```bash
   cd /home/vkaskivuo/Projects/gf2
   git worktree remove .claude/worktrees/agent-b315564a
   git worktree remove .claude/worktrees/agent-1cd3eb09
   # Branches stay; they're archive points
   ```

2. **Plan W3 sub-wave 3a.** Four parallel-dispatchable issues:
   - `d181e95b` (T12) — SIMD bipedal3 kernel. Files: `crates/gf2-kernels-simd/src/bipedal3*.rs` (new).
   - `686ee1b5` (T13) — SIMD dispatch wiring + scalar fallback test. Files: `crates/gf2-algebra/src/permanent/bipedal3.rs` (add runtime dispatch), `crates/gf2-algebra/src/lib.rs` (`OnceLock` SIMD detection).
   - `a7886bd8` (T14) — Multi-word streaming column-sum (n > 64) per R3 (`dev/plans/r3_multi_word_streaming.md`). Files: `crates/gf2-algebra/src/permanent/bipedal3_multiword.rs` (new).
   - `05250df5` (T15) — Rayon parallel permanent_bipedal3 with chunk-size sweep. Files: `crates/gf2-algebra/src/parallel.rs` (extend), `crates/gf2-algebra/benches/permanent.rs` (add parallel group).

   **File-disjoint pairs:** T12+T14 are file-disjoint (separate crates / separate modules). T13 needs T12 + T9's existing path (touches `permanent/bipedal3.rs`). T15 is mostly disjoint. **Pre-flight check:** T13 and T15 both extend gf2-algebra; if they touch the same file (T15 wires into `permanent/bipedal3.rs` via rayon), serialize them.

3. **Dispatch protocol:** Use `scripts/dispatch-worker-worktree.sh` (project-lead skill's). 4 parallel workers if file-disjoint; serialize where not.

4. **W3 sub-wave 3b** (after 3a closes): S1, S2, S3 perf simulations. These consume T12-T15 outputs.

5. **W4 (F_5 / F_7)**: 7 issues. Depends on T12 (SIMD trait) + R1/R2 closed (already done per W0).

6. **W5 (GPU HIP/ROCm)**: 6 issues. Depends on T12.

7. **W6 (Lean verification)**: 4 issues. 2 already have approved sketches (`a0c0a45f` bipedal F_3 vs Fp<3>, `4aaa6e4d` Ryser on FiniteField bounded n ≤ 63 — both `done`). 2 implementation issues remain to be sketched + dispatched.

8. **W7 (Reporting)**: 5 issues for final benchmark artefact + ROADMAP/CLAUDE.md updates + permanent_demo example.

**Process improvements to embed into next-session dispatch prompts:**

- Reference the workspace's `test-support` pattern (`crates/gf2-core/Cargo.toml` lines 28-31, 53-56) when dispatching ANY bench or integration test.
- Budget 15-20 min wall-clock per code-review iteration on bench-target issues; 3-5 min for test/lib-only.
- For Criterion-based perf criteria: explicitly state in the dispatch prompt that the worker must use `measurement_time` low enough that linear-mode `d=1` keeps total under any wall-clock budget.

## Traps — do not repeat these

Carrying forward from handoffs 1-5 (every trap from those handoffs remains in force unless explicitly resolved). Adding session-4's new traps:

### Carried forward (re-stated for emphasis)

All traps from handoff-5 carry forward unchanged. Critical ones to re-emphasise:

- DO NOT paraphrase JIT issue success criteria in dispatch prompts.
- DO NOT trust `cargo bench --no-run` to catch performance budget issues — Criterion's actual sampling at sample_size(10) runs 55·d iters.
- DO NOT instruct workers to "inline this helper because the SSOT module is `#[cfg(test)]`-gated" — point them at the workspace `test-support` pattern instead.

### NEW from session 4 (the long one)

- **DO NOT assume Criterion's `sample_size(10)` means 10 single-call measurements.** Criterion linear mode runs `[d, 2d, ..., 10d]` = 55·d total iterations per cell. To fit a 60 s/cell wall-clock budget at sample_size=10 (the hard minimum), pick `measurement_time` such that `55·d·mean_iter` slightly overshoots `measurement_time` at d=1 (this forces Criterion to pick d=1 and stay within budget). For our T10 bench: `measurement_time(25)` works for the n=28 bipedal cell at 0.59 s/iter (55·0.59 = 32.5 s > 25 s target → d=1, total 33 s, within budget).

- **DO NOT assume slope of `log(time) vs n` matches the source's reported value across different ranges.** For an O(n·k^n) algorithm, the integrated slope over `[n_min, n_max]` equals `ln(k) + mean(1/n)` over the range. Paper's small-`mean(1/n)` at n=24..36 (~0.034) gives slope ~0.727; our larger-`mean(1/n)` at n=8..24 (~0.067) gives slope ~0.760. Both are correct for the same algorithm — the difference is purely the range-dependent 1/n correction. Use a range-adjusted reference, not the source's asymptotic value, when comparing.

- **DO NOT use a hard-coded date string in a file-output path.** Re-runs overwrite the same artefact. Compute the date at runtime from `SystemTime::now()` and format manually (Hinnant civil-from-days is 14 LOC; no chrono/time dep needed). Allow override via env var (e.g. `SA_DATE`) for reproducible pipelines.

- **DO NOT use population variance for sample-derived `std` columns.** Use Bessel-corrected `var = sum((x - mean)^2) / (n - 1)` to match the standard sample-variance definition. With small `n` (e.g. 5) the population-vs-sample variance gap is ~10%.

- **DO NOT trust the reviewer's "test-anchor" constants from another reviewer's prior round.** I (lead) wrote `1_778_803_200` as the 2026-05-11 anchor without verifying via `date -u -d "2026-05-11" +%s`; that's actually 2026-05-15. The reviewer caught it. Always verify anchor constants in tests via the canonical source tool (`date`, `python3 -c "import datetime; ..."`).

- **DO NOT pass through three reviewer-FAIL cycles on the same content issue.** The reviewer agent has stateless reviews; each cycle re-evaluates from scratch. If round N flags a SSOT/correctness/contract issue that the lead's pre-dispatch audit should have caught, the fix is to update the lead's pre-dispatch checklist (memories above), not to keep pushing rework cycles. Specifically: T10's first review caught Lcg-vs-StdRng + n=24 overshoot + SSOT duplication; T10's second review caught the measurement_time / sample_size math error; T10's third review caught n=28 still overshooting (which my own measurement_time tuning should have anticipated). All three could have been pre-empted by a longer dispatch prompt + workspace-pattern check.

- **DO NOT amend the same criterion twice in two separate AskUserQuestion rounds.** T10 criterion 2 was amended on 2026-05-11, again on 11b (after first review), again on 11c (after second review). Each amendment cost a user-question cycle. A pre-dispatch criterion-feasibility audit covering all known constraints (per-cell budget, RNG SSOT, sweep range) would have batched all three into one.

- **DO NOT batch unrelated issues into a single auto-commit.** Session 4 had an auto-loop commit `b9557886` ("chore: add refdb project marker") that absorbed 6 files of substantive code changes (testutil refactor, T10 + T11 bench/test changes) alongside the `.research/project.toml` marker. End state was correct but commit attribution is muddled. If a parallel loop is active during a wave, the lead should commit explicitly BEFORE the parallel loop has a chance to absorb working-tree changes.

## Open questions needing user input

None at session-4 close. W3 dispatch can proceed in session 5 without escalation.

## Reference artefacts

- Epic: `jit issue show ae82bd73` (with 3 amendment blocks).
- Epic design doc: `dev/plans/gf2_algebra_permanent.md` (§2.4 Table 2, §7.2 SSOT amended 2026-05-10).
- R3 design doc: `dev/plans/r3_multi_word_streaming.md` (§2.1.1 added 2026-05-10).
- Sa writeup: `dev/plans/sa_paper_repro_slope.md` (attached to issue 96dcbec4).
- W2 deliverables:
  - `crates/gf2-algebra/src/permanent/{ryser,reference,bipedal3}.rs`
  - `crates/gf2-algebra/src/packed/bipedal3.rs` (Bipedal3::fold_mul_first_n added)
  - `crates/gf2-algebra/src/testutil.rs` (public test helpers)
  - `crates/gf2-algebra/Cargo.toml` (test-support feature)
  - `crates/gf2-algebra/benches/permanent.rs` (T10 bench)
  - `crates/gf2-algebra/tests/permanent_vectors.rs` (T11 integration tests)
  - `crates/gf2-algebra/examples/paper_repro_slope.rs` (Sa harness)
  - `dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-2026-05-11.csv` (Sa output)
- Progress: `dev/active/ae82bd73-progress.json` (refreshed this session).

## Session-4 final metrics

- Issues closed: 4 (T9, T10, T11, Sa).
- Worker rework cycles: 9.
- Lead-direct fixes: 12.
- AskUserQuestion rounds: 5 (7 amendments approved).
- AI code-review gate runs: ~14.
- Sessions ahead in epic: epic at ~49% (26 of 53 children closed; W1 + W2 fully closed; W3-W7 untouched except T16).
- Estimated remaining sessions: 4-6 (W3 ~2 sessions, W4 ~2 sessions, W5 ~1 session, W6 ~1 session, W7 ~1 session — assuming consistent per-issue cycle count). Adjustments likely as W3 surfaces SIMD-specific issues.

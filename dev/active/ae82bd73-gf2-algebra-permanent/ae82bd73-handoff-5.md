# Handoff — Fast matrix permanents over F_3 / F_5 / F_7 via packed bipedal arithmetic (ae82bd73) — session 4

**Date:** 2026-05-11 (session 4 close)
**Session number:** 4
**Prior handoffs:** `ae82bd73-handoff-1.md`, `ae82bd73-handoff-2.md`, `ae82bd73-handoff-3.md`, `ae82bd73-handoff-4.md` (read all in order; traps from each carry forward unless explicitly resolved)

## Current state

- Epic: `ae82bd73` — state: `backlog` (still has open deps).
- Wave in progress: **W2 sub-wave 2c**. T9 closed; T10 + T11 awaiting code-review re-run verdict after a lead-direct SSOT refactor (`test-support` feature + self-dev-dep, modelled on `gf2-core`'s pattern).
- Children summary (epic-level): 24+ closed (was 23 at session-3 close); ~24 still backlog/ready.
  - W1 fully closed (re-confirmed): T1, T2, T3, T4, T5, T6, T16.
  - W2 closed: T7, T8, T9.
  - W2 in flight: T10 (`b315564a`), T11 (`1cd3eb09`).
  - W2 not yet dispatched: Sa (`96dcbec4`) — deps T9 + T10.
- Active claims: T10 + T11 — `agent:claude`.
- Open escalations: none blocking; user pre-approved every amendment this session.
- Progress file: `dev/active/ae82bd73-progress.json` — updated this handoff.
- Worktrees: `worktree-agent-b315564a` and `worktree-agent-1cd3eb09` still present on disk; their work has been merged into `main`. **Cleanup before W3:** `git worktree remove .claude/worktrees/agent-b315564a && git worktree remove .claude/worktrees/agent-1cd3eb09` (the branches stay).

## What just happened (session 4)

### Issues closed this session

1. **T9 (`b0857ae9` — permanent_bipedal3 single-u64 fast path)** — 1 rework cycle + 1 lead-direct fix. Closed PASS.
   - rework cycle 1 (handoff-4 close) FAIL with 3 findings: n bound (`<=63` vs `<=64`), SSOT (inline raw-u64 add/sub/halving fold), oracle (permanent_mod3_reference vs permanent_ryser at n=20/24).
   - This session: user-approved amendments closed findings 1 + 3 (n bound, oracle); rework cycle 2 (Sonnet, mechanical SSOT) added `Bipedal3::fold_mul_first_n` and refactored `permanent_bipedal3` to use `Bipedal3::add/sub/fold_mul_first_n` — eliminating inline raw-u64 arithmetic.
   - Lead-direct: cross-issue SSOT cleanup — consolidated three `random_matrix` helpers into `crate::testutil` (commit `fcbaa003`).
   - Final code-review verdict on session-4 HEAD: PASS.

### Issues in flight this session (state at session-4 close)

2. **T10 (`b315564a` — Criterion bench skeleton)** and **T11 (`1cd3eb09` — Test-vector suite)** — dispatched in parallel from worktrees branched off `f489de81`. Both Sonnet, file-disjoint (`benches/permanent.rs` vs `tests/permanent_vectors.rs`). Both first-review FAIL:
   - T10 findings: (a) Lcg vs StdRng contract mismatch; (b) reference n=24 cell ~80s exceeds 60s/cell budget; (c) `random_matrix_fp3` SSOT duplication (3 places: `testutil.rs`, `tests/permanent_vectors.rs`, `benches/permanent.rs`).
   - T11 finding: same SSOT duplication.
   - Lead-direct fix (commit `b9557886` from harness + `f5e6da69` JIT amendments):
     - Lifted `testutil` from `#[cfg(test)] pub(crate) mod testutil` to `#[cfg(any(test, feature = "test-support"))] pub mod testutil` with `pub` fns, mirroring the gf2-core workspace pattern.
     - Added `test-support = []` feature + self-dev-dep `gf2-algebra = { path = ".", features = ["test-support"] }` to `crates/gf2-algebra/Cargo.toml`.
     - Deleted duplicated `random_matrix_fp3` from bench and integration test; both now `use gf2_algebra::testutil::random_matrix;`.
     - Dropped n=24 from T10's reference sweep (per criterion-4 budget; per user-approved 2026-05-11b amendment).
   - Code-review re-run **in flight** at session-4 close. T10's re-review has been running `cargo bench --bench permanent` for 5+ minutes (the bench's amended n=20 cell takes ~30s × measurement_time(60s) cap). Expected to PASS on the resolution-table review semantics. T11's re-review queued.
   - **Verdict pending at handoff time.** Resume the session by reading `jit gate check-all b315564a` and `jit gate check-all 1cd3eb09` for the latest verdicts.

### User escalations resolved this session

1. **T9 amendments (2026-05-11):** criterion 1 → `mat.cols() <= 63`; criterion 3 oracle → `permanent_mod3_reference` for n ∈ {20, 24}. Recorded as `## Amendment 2026-05-11` in T9 + epic descriptions.
2. **T11 amendments (2026-05-11):** criterion 3 default-tier split (n ∈ {4, 8, 12}); criterion 4 `permanent_mod3_reference` oracle for n ∈ {20, 24}; criterion 6 default-tier-only 5 s/test (slow-tier separate). Recorded as `## Amendment 2026-05-11` in T11.
3. **T10 amendments (2026-05-11):** criterion 2 split sweep ranges (reference n=8..24 → later trimmed to n=8..20; bipedal n=8..36); criterion 3 RNG: `StdRng` → `gf2_core::rng::Lcg via gf2_algebra::testutil`. Recorded as `## Amendment 2026-05-11` and `## Amendment 2026-05-11b` in T10.

### Lead-direct minor edits (no worker dispatch) this session

- **T9 close-out:** `random_matrix` test-helper consolidation into `crate::testutil` (commit `fcbaa003`). Workspace SSOT before T10/T11 land.
- **T10 + T11 SSOT cleanup:** test-support feature exposing `gf2_algebra::testutil` publicly (commits `b9557886` and `f5e6da69`). Modelled on `gf2-core`'s workspace pattern (see `crates/gf2-core/Cargo.toml` lines 28-31, 53-56).

## What to do next

**Immediate (session 5 start):**

1. Resume by reading `jit gate check-all b315564a` and `jit gate check-all 1cd3eb09`. If both pass: `jit issue update b315564a --state done`, `jit issue update 1cd3eb09 --state done`, commit JIT state.
2. If either fails: read the verdict, apply a narrowly scoped lead-direct fix (this is rework cycle 2 of 2 for both; escalate to user if a third rework would be required).
3. After both close: dispatch Sa (`96dcbec4 paper Table 2 repro`) as a single Sonnet worker. Spec is already detailed in the issue; deps T9 + T10 will be `done`.
4. Cleanup the W2 worktrees: `git worktree remove .claude/worktrees/agent-b315564a && git worktree remove .claude/worktrees/agent-1cd3eb09` (branches kept for archival).
5. After W2 fully closed (T10, T11, Sa all done), plan W3 sub-wave 3a: T12 (SIMD bipedal3 kernel, `d181e95b`) + T13 (SIMD dispatch, `686ee1b5`) + T14 (multi-word streaming, `a7886bd8`) + T15 (rayon parallel, `05250df5`). Parallel-via-worktree dispatch; T12 + T14 are file-disjoint (`gf2-kernels-simd/` vs `gf2-algebra/permanent/`).
6. W3 sub-wave 3b: S1 (`b69ce7c8` headline 50× perf vs `permanent_mod3_reference`), S2 (parallel scaling), S3 (multi-word correctness vs T9). These consume T12/T13/T14/T15 outputs and the criterion bench from T10.

**Process improvements (lessons embedded into next-session dispatch):**

- **Check workspace test-support patterns BEFORE dispatching a worker.** This session burned 1 rework cycle each on T10 and T11 because the dispatch prompts instructed inline copies of `random_matrix_fp3` "because testutil is `#[cfg(test)]`-gated." gf2-core's `test-support` feature pattern WAS the right answer; would have been caught by `grep -rn 'test-support' crates/*/Cargo.toml` at dispatch time. **Memory candidate:** `feedback_check_workspace_test_support_pattern` — when dispatching a bench or integration-test target, check if the workspace has a `test-support`-style feature before instructing "inline because cfg(test)-gated".
- **Reviewer literal contract enforcement.** T10's criterion 3 wording "fixed seeded `StdRng`" was paraphrasing per the breakdown, but the reviewer reads it literally. Two contract resolutions both work: (a) write the code to match the literal criterion; or (b) amend the criterion to match project SSOT. This session chose (b) with a long honest reasoning chain (committed-seed reproducibility across rand version bumps, Charon/Aeneas extractability, dep minimalism, SSOT consistency, statistical adequacy for P ∈ {3, 5, 7}). The user pushed back twice asking whether there was a real reason for Lcg vs StdRng — answer ended up being "yes, several concrete reasons" but the lead should have been ready to defend the choice without prompting. **Memory:** existing `feedback_quote_jit_criteria_verbatim` already covers the lesson; reinforce it.
- **Long-running gate runs.** The T10 code-review on the rework HEAD invoked `cargo bench --bench permanent` (without `--no-run`!) which is a 5+ minute live bench run. The reviewer agent decided to validate the bench by running it. This is far longer than the typical 3-min code-review cycle. If repeated on every iteration, gate runs become the bottleneck. Consider amending `scripts/code-review-prompt.md` to instruct the reviewer to use `cargo bench --no-run` only for bench-targeting issues. **NOT done this session** — flagging as a potential follow-up if the latency becomes a problem.

## Traps — do not repeat these

Carrying forward from handoff-1, handoff-2, handoff-3, handoff-4 (every trap from those handoffs remains in force unless explicitly resolved). Adding session-4's new traps:

### Carried forward (re-stated for emphasis)

All traps from handoff-4 carry forward unchanged. Critical ones to re-emphasise:

- **DO NOT paraphrase JIT issue success criteria in dispatch prompts.** Two cycles this session burned because of this. T10's criterion 3 says "StdRng"; lead's prompt told worker to use Lcg "because it's the SSOT". Worker complied with the prompt; reviewer rejected for not matching criterion. The fix is either to comply with the literal text OR amend the criterion BEFORE dispatch. Both rounds need an upfront `AskUserQuestion` if there is any tension between the literal criterion and what the lead's project-wide SSOT instinct says.
- **DO NOT trust `cargo bench --no-run` to catch performance budget issues.** T10's bench compiles fine with `--no-run`, but the actual cell wall-clock time is the criterion-4 contract. The reviewer runs the bench live; if a cell exceeds the documented budget, that's a fail. Drop n=24 from a sweep upfront if your own annotated estimate says "borderline".
- **DO NOT trust the worker to consult the workspace's `test-support`-style feature pattern.** Bench and integration-test compilation units cannot reach `#[cfg(test)] mod testutil`. The workspace's existing answer is in `crates/gf2-core/Cargo.toml` lines 28-31 + 53-56. Reference that pattern in EVERY dispatch prompt that creates a bench or integration test in a Cargo workspace.

### NEW from session 4

- **DO NOT instruct workers to "inline this helper because the SSOT module is `#[cfg(test)]`-gated".** That advice was wrong this session — the workspace has a `test-support` feature pattern that exposes `pub mod testutil` to benches and integration tests under `#[cfg(any(test, feature = "test-support"))]`. Two reworks burned. Future dispatches creating bench / integration-test targets MUST cite this pattern and instruct the worker to extend `testutil` if missing surfaces are needed.

- **DO NOT amend the same criterion twice via separate `AskUserQuestion`s in the same session.** T10 criterion 2 was amended on 2026-05-11 (sweep ranges) and again on 2026-05-11b (drop ref n=24). T10 criterion 3 was amended on 2026-05-11b only (StdRng → Lcg). Both rounds of amendments cost a separate user-question cycle. A pre-dispatch criterion-feasibility audit comparing the literal criterion against measured per-cell costs would have batched both into a single amendment.

- **DO NOT assume code-review wall-clock is ~3 minutes.** This session's T10 review took 12+ minutes because the reviewer chose to run `cargo bench --bench permanent` live (not `--no-run`). Bench-target issues have a much longer code-review tail than tests-only issues. Budget for a 10-15 min wait per bench-related code-review iteration.

- **DO NOT confuse `git status` showing untracked `.research/project.toml` with a worker leak.** The research-librarian skill creates this marker outside any worker workflow; the leak-check script flags it as "disappeared between snapshot and now" because the harness reorganises that file across sessions. Not a real leak; safe to ignore (but verify with `git log -- .research/` if there is doubt).

- **DO NOT lose track of who committed.** This session had an auto-loop commit `b9557886` ("chore: add refdb project marker for gf2") that picked up MY staged working-tree changes (testutil refactor, T10 bench, T11 tests) into a single commit. That commit was labeled as a chore but contained 6 files of substantive code changes. My subsequent explicit commit (`f5e6da69`) only had the JIT-state files because everything else had already been picked up by the harness. End state was correct, but the commit attribution is muddled. If a similar auto-loop runs during a wave, the lead's commit may be effectively empty.

## Open questions needing user input (next session)

None at session-4 close. All amendments approved; T10 + T11 just need their re-review verdicts to land.

If T10's re-review FAILS (rework cycle 2 of 2), escalation per policy entry 5 would be required to:
- accept a third rework cycle, or
- take over manually for a small remaining fix, or
- reject the issue.

T11 is at rework cycle 1 of 2; one more cycle is available without escalation.

## Reference artefacts

- Epic: `jit issue show ae82bd73` (description includes `## Amendment 2026-05-10` for criterion 2 oracle range + `## Amendment 2026-05-11` for criterion 2 oracle for n in {20, 24}).
- Epic design doc: `dev/plans/gf2_algebra_permanent.md` (§7.2 amended 2026-05-10).
- R3 design doc: `dev/plans/r3_multi_word_streaming.md` (§2.1.1 added 2026-05-10).
- W2 deliverables landed this session:
  - `crates/gf2-algebra/src/permanent/bipedal3.rs` (T9 — refactored to use Bipedal3 SSOT API per finding 2)
  - `crates/gf2-algebra/src/packed/bipedal3.rs` (T9 — added `Bipedal3::fold_mul_first_n`)
  - `crates/gf2-algebra/src/testutil.rs` (lead-direct cross-issue SSOT consolidation; later upgraded with running doctests)
  - `crates/gf2-algebra/Cargo.toml` (test-support feature + self-dev-dep)
  - `crates/gf2-algebra/src/lib.rs` (testutil cfg gating)
  - `crates/gf2-algebra/benches/permanent.rs` (T10 — Criterion bench skeleton)
  - `crates/gf2-algebra/tests/permanent_vectors.rs` (T11 — test-vector suite)
- Progress: `dev/active/ae82bd73-progress.json` (refreshed this session).
- Memory additions (2026-05-11): candidate `feedback_check_workspace_test_support_pattern` (not yet written).

## Session-4 metrics

- Issues closed: 1 (T9) — 1 rework + 2 lead-direct fixes.
- Issues in flight at close: 2 (T10, T11 — code-review re-run pending; lead-direct SSOT fix already landed).
- User escalations resolved: 3 (T9 amendments, T11 amendments, T10 amendments).
- AI code-review gate runs (gpt-5.4): ~5 (T9 ×1, T10 ×2, T11 ×1+ in flight). One particularly long run (T10's bench validation, 12+ min wall-clock).
- Sessions ahead in epic: epic at ~47% (24 of 51 children closed; W1 + most of W2 closed; W3-W7 untouched).

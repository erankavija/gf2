# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 11

**Date:** 2026-05-26 / 2026-05-27 (multi-day session)
**Session number:** 11
**Prior handoffs:** sessions 1–10 in `dev/active/026fc832-handoff*.md`. Read every prior handoff's **Traps** section — all carry forward.

## Current state

- Epic: `026fc832` — state: `backlog` (assignee `agent:project-lead`)
- Wave 7a: **DONE** (6823c8a0 closed all 3 gates; merged)
- Phase 6b (695350fd): **DONE** (closed all 3 gates after R2 paired re-bench refuted the GF(7)/n=64 regression flag)
- **Wave 7b: DONE** (all 3 implementations closed after R1 rework — 869ce43b + 6613abf4 closed on first R1 review pass; 8df0c501 needed an R2 lead-direct trial-file fix for GF(31)/n=64)
- Epic dep DAG: 15 children (10 done, 4 ready, 1 in_progress). New this session: `695350fd`, `74ba1cdc`, `68db401b`, `6a7d4c8e`, `9138d86c`, `d36cc414`.
- Open escalations: **none unresolved**. Session 11 escalations all answered.

### Remaining work (4 ready follow-ups + 1 in_progress + 2 transitive)

| Issue | State | Path |
|---|---|---|
| `6a7d4c8e` | ready | M31 echelon: wire m31_batch_dot_fn into gemm_axpy_into_view (closes 869ce43b row 33) |
| `9138d86c` | ready | GF(65521)/n=64 blocked solve kernel (closes 6613abf4 rows 55, 56) |
| `d36cc414` | ready | GF(251)/n=64 borderline investigation (covers wave-7b rows 23, 37, 40, 51, 52) |
| `68db401b` | ready | u16-lane PLE base-case kernel (Phase 6d) |
| `0749dbad` | ready (transitive via 695350fd) | f64 GEMM cascade (Phase 6e) |
| `98336ab4` | in_progress (blocked on `0749dbad`) | GF(p) fgemm n=4096 re-bench |
| `b0fa00af` | in_progress | v2 SOTA scorecard re-publish (Wave 8 — depends on everything above) |

## What happened this session

### 6823c8a0 (panelized PLE) — closed
- R3 worker on worktree-agent-6823c8a0-r3 completed in prior session; ff-merged.
- R4: lead-direct fix replacing 6 deterministic `test_ple_panelized_boundary_sweep_*` tests + helper `boundary_sweep_fp<P>` with proptest! variants.
- R5: lead-direct restructure — proptest macro drives seed variance, test body **exhaustively** iterates all 63 boundary pairs. All 6 prime sweeps PASS at ~40ms each.
- Code-review PASS at commit `66c1b762` (after the DAG-wiring commit landed it ran in 61s clean). cargo-ci PASS. doc-review attested. Closed at commit `44dc7614`.

### 695350fd (fp_medium BLIS) — closed
- R2 paired re-bench (user-approved) on commits `75826c09` (state A) vs current main (state B) refuted the R1 -6.2% GF(7)/n=64 regression flag.
- Median delta GF(7)/n=64: -2.00 % (well within 5% threshold). 10-trial extension for GF(251)/n=64 gave -4.43 % median.
- All 4 unmodified `fp_small_f32` cells within 5%. SC#2 satisfied without amendment.
- Also fixed SC#3 evidence-doc stale references (deleted-test names from 6823c8a0 R5 cleanup).
- Closed at commit `38387525`.

### DAG wiring (user-approved)
- Filed 3 new follow-up tasks under epic 026fc832:
  - `6a7d4c8e` — M31 echelon: wire `m31_batch_dot_fn` into `gemm_axpy_into_view` (closes 869ce43b row 33)
  - `9138d86c` — GF(65521)/n=64 blocked solve kernel (closes 6613abf4 rows 55, 56)
  - `d36cc414` — GF(251)/n=64 borderline investigation (covers 869ce43b row 23, 8df0c501 rows 37+40, 6613abf4 rows 51+52)
- Wired all 3 as direct deps of epic 026fc832. None of them is currently in_progress.
- Also wired during this session: `695350fd`, `74ba1cdc`, `68db401b` (user decision earlier in session). The wave-7b issues themselves do NOT depend on the follow-ups (closure via [aspirational] amendment).

### Wave 7b (R0 → R1) — in flight
- R0 workers (869ce43b echelon, 8df0c501 invert, 6613abf4 solve) all completed with partial coverage:
  - 869ce43b: 14/18 PASS, 4 cells routed (rows 23, 24, 25, 33)
  - 8df0c501: 8/11 PASS, 3 cells routed (rows 37, 39, 40)
  - 6613abf4: 7/13 PASS, 6 cells routed (rows 51-56)
- User-approved amendment (session 11 AskUserQuestion: "Investigate GF(251)/n=64 borderline first; amend the rest"):
  - 5 cells `[aspirational]` inheriting 6823c8a0 GF(251)/n=256 PLE gap: 869ce43b rows 24, 25; 8df0c501 row 39; 6613abf4 rows 53, 54.
  - 5 cells routed to `d36cc414`: 869ce43b row 23, 8df0c501 rows 37, 40, 6613abf4 rows 51, 52.
  - 1 cell routed to `6a7d4c8e`: 869ce43b row 33.
  - 2 cells routed to `9138d86c`: 6613abf4 rows 55, 56.
- Amendments applied to issue descriptions at commit `06cef9fe`. Original SC#4 preserved in "Original Success Criteria" subsection.
- cargo-ci: PASS on all 3 (after re-running serially — see Traps).
- code-review on amended state: **FAIL on all 3** with specific findings:
  - 869ce43b: CSV missing GF(31)/n=64; non-regression sweep used wrong baseline (rref vs pluq); stale docstring at ple.rs:37-38; also flaky bipedal3 timeout (out of scope).
  - 8df0c501: bench used Criterion default (not CCX1 flock-pinned); n=1024 missing.
  - 6613abf4: rank-deficient proptest sweep missing; evidence doc still routes rows 51-56 to 615db3b9 (stale).
- R1 reworks dispatched (sonnet, worktrees, background): see "Current state" above.

## What to do next

In order of priority:

- [x] ~~Wait for 3 R1 rework workers~~ — all completed; merged; gates passed.
- [x] ~~Close wave 7b R1 issues~~ — 869ce43b, 8df0c501, 6613abf4 all closed.
- [ ] **Dispatch the 3 small/medium follow-up tasks in parallel worktrees:**
  - `6a7d4c8e` — M31 echelon GEMM-axpy dispatch (single-file kernel change in `gfp/simd_ops.rs` + `matrix.rs`)
  - `9138d86c` — GF(65521)/n=64 blocked solve kernel (likely a new dispatch path in `triangular.rs`)
  - `d36cc414` — GF(251)/n=64 borderline investigation (profile + write evidence doc; may not modify production code)
  - These could all run in parallel via the worktree-dispatch protocol. Sonnet model fine for the first two; opus may be appropriate for d36cc414 since it's an investigation requiring judgment.
- [ ] **Dispatch Phase 6d (`68db401b`) + Phase 6e (`0749dbad`)** — each is multi-day and probably needs its own breakdown task before implementation. Both are kernel work.
- [ ] **98336ab4 re-bench + close** — after `0749dbad` lands. Pre-existing worktree at `.claude/worktrees/agent-98336ab4` is preserved.
- [ ] **Investigate flaky bipedal3 test** `permanent::bipedal3::tests::test_simd_vs_scalar_n24` (was flagged in 869ce43b R0 review but didn't recur in R1). Likely needs `#[ignore = "slow"]` if it consistently runs near 5s. NOT in 869ce43b scope — file a separate JIT task.
- [ ] **Wave 8 (`b0fa00af` v2 scorecard)** — after the follow-ups + Phase 6d/6e + 98336ab4 close. Holds the bulk of the "close the loop" work.
- [ ] **Epic close (Section 10)** — after wave 8 + follow-ups close, OR after user decides which follow-ups to defer to a successor epic. The cumulative complexity (4 distinct architectural gaps now tracked: GF(251)/n=256 Schur-update, GF(251)/n=64 borderline, M31 GEMM-axpy dispatch, GF(65521)/n=64 solve) may justify a successor epic decision.

## Traps — do not repeat these

**Carry forward** (link, don't copy): session 1–10 handoffs' Traps. All still in force.

**New session 11 traps:**

- **Parallel `jit gate pass <id> cargo-ci` races** — Running cargo-ci for multiple issues in parallel on the same checkout causes flaky test failures (exit 100 with empty stderr and "test: FAILED" but no test name). Sequential runs PASS on the same commit. Memory: [[parallel-cargo-ci-flaky]]. **Workaround:** serialize cargo-ci runs; parallel works only across separate worktrees.

- **`pgrep -f <pattern>` self-matches in wait loops** — `until ! pgrep -f 'ai-review.sh'; do sleep 30; done` never exits because the pgrep's own argv contains the literal pattern; the Bash tool's zsh wrapper hosts the pgrep so it sees itself. **Workaround:** rely on the Bash tool's run_in_background task-notification, NOT polling. Memory: [[pgrep-self-match]].

- **MCP `jit_gate_pass` times out at 600s** — The MCP transport hard-times-out at 600s; the underlying reviewer regularly takes 5–10 min. MCP returns TIMEOUT but the reviewer keeps running in the background (orphan) — no gate state is recorded. **Workaround:** always use shell `jit gate pass <id> code-review` via the Bash tool's run_in_background. Updated memory: [[code-review-via-jit-cli]].

- **Workers self-designate cells as [aspirational]** — The 8df0c501 R0 worker reported 3 cells as "ASPIRATIONAL" in its bench table. Workers cannot self-designate per `feedback_no_autonomous_amendments` — only the lead can apply [aspirational] amendments, only after user approval. **Workaround:** include in dispatch prompts: "If new cells need [aspirational] designation, FLAG them in your report — do NOT amend the issue description yourself."

- **Stale evidence doc references to closed predecessor issues** — The 6613abf4 R0 worker routed 6 cells to `615db3b9` (already done/closed). The reviewer correctly flagged the doc as stale once the lead-side amendment named the actual follow-up tasks (`d36cc414`, `9138d86c`). **Workaround:** dispatch prompts should specify the EXACT routing destination IDs upfront if known, OR the lead should grep the worker's evidence doc post-merge for any closed-predecessor IDs before running code-review.

- **Workers branched before claim commit show stale .jit/issues/<id>.json deltas** — When the lead claims issues AFTER `dispatch-worker-worktree.sh` creates the branches, the worker branches don't have the claim commit. `git diff main..worker` will show issue state going from "in_progress" → "ready" — that's NOT autonomous amendment, just rebase-clean state. **Workaround:** rebase the worker branch onto current main before reviewing the diff.

- **`SQUARE_SIZES_SMALL_PRIME_WITH_N64` doesn't exist at pre-695350fd commits** — When doing paired baseline benches against state A = commit before 695350fd R1 landed, GF(127)/n=64 and GF(241)/n=64 bench cells don't exist (added by R1). The paired bench can only cover GF(7), GF(31), GF(251), GF(65521) at n=64. Note this limitation in evidence docs.

- **`bench-flock.sh` needs `--features simd`** — The bench harness uses `set_route_a_gf251_enabled` / `set_route_c_gf251_enabled` functions gated on `feature = "simd"`. Without the feature, `cargo bench --no-run` fails with E0425. Workers should always pass `--features simd` for bench builds.

## Open questions needing user input

None unresolved. The session 11 escalations were all answered:
- DAG wiring (in: wire 695350fd/74ba1cdc/68db401b to epic) — answered, applied.
- Wave 7b closure plan — answered, applied (investigate borderline + amend the rest).
- 695350fd SC#2 closure — answered, applied (paired re-bench).

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (last full update session 6; session-7/8/9/10/11 changes in commit history + handoffs)
- Session 1–10 handoffs: `dev/active/026fc832-handoff*.md`
- Closed this session: 6823c8a0 (panelized PLE), 695350fd (fp_medium BLIS)
- In flight: 869ce43b R1, 8df0c501 R1, 6613abf4 R1 (worktree branches)
- Filed this session: 6a7d4c8e (M31 echelon), 9138d86c (GF(65521)/n=64 solve), d36cc414 (GF(251)/n=64 investigation)
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Reference host: AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512

## Active worktrees + branches at session-11 end

```
/home/vkaskivuo/Projects/gf2                                     9a9322d6 [main]
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-30e98ef1-d6 (unrelated; preserved)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-9480f8a6    (unrelated; preserved)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-98336ab4    (in_progress; preserved for eventual re-bench after Phase 6e)
```

All wave-7b worktrees cleaned up at session end.

Main HEAD at end of session 11: `9a9322d6` (chore commit closing 8df0c501 R2).

## Session 11 commit chain (selected high-impact)

- `530d779d`: 6823c8a0 R3 (scalar-oracle proptests, merged from worktree)
- `bcbbf771`: 695350fd R1 (re-bench n=64 + § 6.3 proptest citation, merged from worktree)
- `d509e436`: 6823c8a0 R4 (proptest! boundary-sweep with random-pair sampling)
- `f8f0c034`: 6823c8a0 R5 (proptest body exhaustively iterates all 64 pairs)
- `530d779d → 44dc7614`: 6823c8a0 closure
- `66c1b762`: wire 695350fd/74ba1cdc/68db401b into epic DAG
- `c91f8a57`: 695350fd R2 paired re-bench (refutes GF(7)/n=64 regression flag)
- `38387525`: 695350fd closure
- `06cef9fe`: wave 7b amendments + 3 follow-up tasks filed
- `adc7ce20`: record wave 7b R0 gate runs (cargo-ci PASS, code-review FAIL)
- `a4c04a91`: mid-session handoff (this doc, initial version)
- `bc717f5f`: 869ce43b R1 — GF(31)/n=64 cells + BLOCKED_BACK_SUB_MIN_DIM=128 threshold + same-op non-regression bench
- `dc01b954`: 8df0c501 R1 — CCX1-pinned 5-trial medians + n=1024 cells + raw trial files
- `a972cc35`: 6613abf4 R1 — rank-deficient proptest sweep + evidence doc routing fix
- `83854b3a`: close 869ce43b + 6613abf4 (all gates pass on R1 first review)
- `85949af3`: 8df0c501 R2 — add missing GF(31)/n=64 trial files (single-finding fix)
- `9a9322d6`: close 8df0c501 (all gates pass on R2)

**Session 11 summary**: 5 issues closed (6823c8a0, 695350fd, 869ce43b, 8df0c501, 6613abf4). 3 follow-up tasks filed (6a7d4c8e, 9138d86c, d36cc414). 4 new memory entries (parallel-cargo-ci, pgrep-self-match, MCP-code-review-timeout, worker-self-amendment guard).

The remaining work — 4 ready follow-ups + Phase 6d/6e + 98336ab4 + Wave 8 + epic close — represents multiple future sessions. Cumulative complexity continues to grow (4 distinct architectural gaps now tracked: GF(251)/n=256 Schur-update, GF(251)/n=64 borderline, M31 GEMM-axpy dispatch, GF(65521)/n=64 solve). The user's pattern of "investigate before amend" is keeping the contract honest while making forward progress.

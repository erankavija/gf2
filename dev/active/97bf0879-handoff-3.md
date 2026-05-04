# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 3

**Date:** 2026-05-04
**Session number:** 3
**Prior handoffs:**
- `dev/active/97bf0879-handoff.md` (session 1, 2026-04-30) — Wave 1 closure.
- `dev/active/97bf0879-handoff-2.md` (session 2, 2026-05-04 morning) — Wave 2 closure + Wave 3 dispatch plan.
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md` — read at minimum the **Traps** sections of `babcf05e-handoff-5.md`. All prior-session traps remain in force.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Wave 3 status: **partial close (4/6 closed).** Closed: `0fd48627`, `c3e79272`, `609855d9`, `3b762764`. Open: `9a715d75`, `a3412e15` (gated on new impl tasks per user decision).
- Bench-day reconciliation completed 2026-05-04 — pinned bench produced `benchmarks/results/20260504T135723Z.csv`, both 609855d9 and 3b762764 closed cleanly on the post-bench-day evidence.
- New scope work: 3 new tasks created from user-approved Wave-3 escalation:
  - `b13799ac` — Build GF(2^32) matmul reference harness (story `2c7548ae`); blocks closure of `9a715d75`.
  - `2403c054` — gf2-core `SpBitMatrix::matmul` (GF(2) sparse-sparse) (story `54fd3f0b`); blocks closure of `a3412e15`.
  - `eb57f944` — gf2-core `SparseFieldMatrix` sparse-matmul + sparse-rref over GF(p) and GF(2^m) (story `54fd3f0b`); blocks closure of `a3412e15`.
- Open escalations: none currently blocking.
- Branch state: `main` will be at the post-bench-day commit when this session ends.
- Progress file: `dev/active/97bf0879-progress.json` (12-wave plan; updated by lead at session close).

## What just happened (session 3 narrative)

### Wave 3 dispatch + rate-limit truncation
- Dispatched all 6 Wave 3 issues in parallel worktrees per `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`. 4 workers (those producing benchmark numbers) received the additional rule: do not run `./benchmarks/run.sh` from a worktree (host CPU contention).
- 4 of the 6 workers hit the user's Anthropic quota mid-task (limit reset at 13:30 EEST). Two completed cleanly first (`9a715d75`, `a3412e15`); one wrote a substantial WIP that I lead-preserved + finalized (`c3e79272`); one wrote raw artefacts but no doc (`0fd48627` — re-dispatched as a continuation worker after limit reset); one wrote nothing (`3b762764` — re-dispatched fresh after limit reset); one committed cleanly (`609855d9`).

### `git restore .jit/` lost worker `jit doc add` state
- Each worker's `jit doc add` from inside its worktree leaks `.jit/issues/<id>.json` mods into main (per Wave-2 trap, this is benign but distinct from a source-file leak). When I tried to clean main's working tree before FF-merging worker branches, I dropped these mods — and the workers' commits did NOT include them, so the doc-attach state was lost.
- **Recovery:** re-applied every worker's `jit doc add` calls lead-direct after FF-merge. Re-claimed all 6 issues to `agent:project-lead` (the original `agent:claude-wt-<id>` claims were also lost in the same drop). Three subsequent `chore(jit:97bf0879)` commits restored the JIT state.

### Code-review failures + rework
- Initial code-review run: 5 of 6 issues FAILED.
  - `c3e79272` PASS first try (NTL minpoly excluded under existing protocol § 8 class).
  - `0fd48627` FAIL → mechanical fixes (CSV row citations, `4c0d0202` claim) → PASS r2.
  - `9a715d75` + `a3412e15` FAIL → user-approval needed for proposed exclusions; user escalation answered the closure path.
  - `609855d9` FAIL → `MEASUREMENT GAP — GF(31)`; user picked "lead runs one-off pinned bench".
  - `3b762764` FAIL → criterion #1 PARTIAL (worker self-report); user picked "fresh pinned dense-LA bench day".

### User escalation #1 (4-question) outcomes
1. **9a715d75 closure:** "Harness GF(2^32) instead (extend scope)." → New task `b13799ac` filed.
2. **609855d9 closure:** "Lead runs one-off pinned GF(31) bench (Recommended)." → In-flight bench day adds GF(31) row; closes 609855d9.
3. **3b762764 closure:** "Run fresh pinned dense-LA bench day." → Same in-flight bench captures all cells; closes 3b762764.
4. **a3412e15 closure:** "File gf2-core sparse-impl follow-ups inside epic." → New tasks `2403c054` + `eb57f944` filed.

### User escalation #2 (2-question) outcomes
After 4 code-review cycles on 9a715d75, the strict reviewer kept rejecting criterion #1 because "GF(2^32) lane is delegated to b13799ac, not actually selected here." Same expected for a3412e15 and its 5 self-reference cells (impl tasks pending). User picked:
- **9a715d75:** Keep open until `b13799ac` closes.
- **a3412e15:** Keep open until `2403c054` + `eb57f944` close.

JIT deps wired:
- `9a715d75 → b13799ac`
- `a3412e15 → {2403c054, eb57f944}`
- `47698404 (Wave 10 sparse scorecard) → {2403c054, eb57f944}`
- `dece4e73 (Wave 12 aggregate) → b13799ac`
- `4c0d0202 (Wave 4 target matrix) → {9a715d75, a3412e15, c3e79272, 609855d9, 0fd48627, 3b762764}` — Wave 4 will dispatch only after all Wave 3 issues close.

### Protocol Amendment 2 landed
Per the user's approval of new exclusion classes + sparse operation values, `dev/plans/sota_reference_acceptance_protocol.md` got Amendment 2 (new § 14, renumbered legacy §14 → §15):
- § 7 *CSV schema* operation list extended with `sparse-matmul`, `sparse×dense`, `sparse-elim`.
- § 9 *Exclusion class registry* gained `not-yet-harnessed` and `no-independent-oracle`.
- Downstream `analyze.py` validator update is owned by `47698404`.

### In-flight pinned bench day
At session-end, a full pinned-container fflas-ffpack bench run is in flight (background bash task `b8aeqwmly`). Modifications to `benchmarks/reference/fflas_bench.cpp`:
- Added `GF(31)` driver (`Givaro::Modular<int64_t> F(31)`) mirroring the existing `GF(7)` block, with seed `master_seed ^ 0x44ULL`.

The bench-day captures:
- All 5 named primes for 609855d9: GF(7), **GF(31)** (new), GF(251), GF(65521), Mersenne31.
- All dense-LA operations for 3b762764: fgemm, pluq, echelon, invert, solve, charpoly, minpoly across full-rank + rank-deficient regimes (the fflas_bench already emits both per `2026-04-26-reference.csv`).

When the bench completes:
1. New CSV at `benchmarks/results/<timestamp>.csv` containing all rows including the new GF(31) ones.
2. Lead extracts the GF(31) row, appends to `2026-04-26-reference.csv` (or creates a 2026-05-04 supplement), updates `609855d9`'s evidence doc.
3. Lead extracts post-PPC dense-LA rows, updates `3b762764`'s evidence doc — the rows for PLE/echelon/invert/solve are expected to be statistically identical to the 2026-04-26 baseline because gf2-core has not re-implemented those code paths post-PPC.
4. Re-run code-review on both. If pass, run doc-review (manual lead attestation). Mark both done.

## Wave 3 — closed-issue evidence ledger

| Short ID | Title | Final commit | Final state |
|---|---|---|---|
| `0fd48627` | Profile post-PPC GF(2) M4RI gap | `1e7d7d1` (worker) + `166afbd` (lead-direct citations) | DONE |
| `c3e79272` | Build charpoly minpoly reference lane | `e84ecc4` (lead-finalized worker WIP) | DONE |
| `609855d9` | Classify GF(p) gap by prime family | `42b584d` (worker) + post-bench-day GF(31) reconciliation + R2 review pass | DONE |
| `3b762764` | Re-run dense LA post-GEMM scorecard | `5772839` (worker) + post-bench-day fresh-CSV + worker-scope clarification on baseline-aggregation framing + R5 review pass (`a1523ea`) | DONE |
| `9a715d75` | Select GF(2^m) reference lane | `92c838c` (worker) + 4 lead-direct re-frame commits | OPEN — blocks on `b13799ac` |
| `a3412e15` | Select sparse benchmark corpus and references | `9aaaa2e` (worker) + 2 lead-direct re-frame commits | OPEN — blocks on `2403c054` + `eb57f944` |

## What to do next

In priority order:

- [x] ~~Confirm bench day completed cleanly.~~ (done — `benchmarks/results/20260504T135723Z.csv`)
- [x] ~~Update 609855d9 evidence doc with the GF(31) row.~~ (done — closed 2026-05-04)
- [x] ~~Update 3b762764 evidence doc with the post-PPC dense-LA rows.~~ (done — closed 2026-05-04 after R5)
- [ ] **Decide whether to dispatch `b13799ac` / `2403c054` / `eb57f944` in this session or next.** They are substantial implementation tasks (1-3 days each). The immediate Wave-3 critical path is now resolved (4/6 closed); the remaining 2 (`9a715d75` + `a3412e15`) wait on these impl tasks but do **not** block Wave 4 — `4c0d0202` (Wave 4) depends on all 6 Wave 3 issues, so Wave 4 dispatch must wait. Recommended path: dispatch `b13799ac` in next session as a focused implementation wave (it's also blocked on `dece4e73`'s GF(2^32) ring oracle harness, so order is `dece4e73` → `b13799ac` → close `9a715d75`). `2403c054` + `eb57f944` similarly chain on `47698404` (analyze.py protocol §9 update).

## Wave 4 readiness

- `4c0d0202` (Publish SOTA target matrix design doc) is wired to depend on all 6 Wave 3 issues. It cannot dispatch until `9a715d75` and `a3412e15` close, which in turn requires `b13799ac` / `2403c054` / `eb57f944` to land.
- The dependency chain that gates Wave 4:
  - `b13799ac` blocks on `dece4e73` (Wave-12 ring-oracle harness — actually classified as Wave-2-style infrastructure, currently `ready`).
  - `2403c054` + `eb57f944` block on `47698404` (Wave-10 sparse scorecard analyze.py update).
- Realistic Wave 4 dispatch window: after `b13799ac` / `2403c054` / `eb57f944` close (3-7 days of impl + review across two parallel sub-waves).

## Traps — do not repeat these

Carry-forward from earlier handoffs (still in force):
- All traps from `babcf05e-handoff-5.md` and `97bf0879-handoff{,-2}.md` remain active. Re-read them on session resume.

New traps from session 3:

1. **`git restore .jit/` after a worker's `jit doc add` leak silently drops claim state and doc-attach state.** Worker `jit doc add` from inside a worktree dirties main's `.jit/issues/<id>.json` instead of committing into the worker branch. `git restore` then erases the doc-attach mutation AND the original `jit issue claim` state — leaving the issue back at `ready, unclaimed` and the docs unattached. Recovery requires re-running every `jit doc add` from main AND re-claiming each issue. **Workaround for next session:** before `git restore .jit/`, capture the leaked mods (`git diff .jit/ > /tmp/preserve.patch`), restore, then re-apply the patch. Or just commit the leak first as a chore commit, then rebase worker branches over it.

2. **`--skip-build` reuses a stale image that may predate Wave-2 library pins.** First attempt at the GF(31) bench used `./benchmarks/run.sh --skip-build --skip-m4ri` which reused `gf2-bench:ref` from the 2026-04-26 baseline. That image has no linbox/m4rie/ntl/flint, so `make -B` for `linbox_bench` fails. **Workaround:** drop `--skip-build` whenever a Wave-2-or-later library is in scope (or when `image.lock` shows a `[libs.<lib>]` block the cached image lacks).

3. **Strict AI reviewer rejects "candidate pool" framing for criterion #1 of a selection issue.** When 9a715d75 framed `(matmul, GF(2^32))` as "selected = {NTL, FLINT} candidate pool" with the final pick delegated to `b13799ac`'s harness work, the reviewer (claude --model sonnet -p) repeatedly rejected this as "not actually selected." After 4 review cycles, the user picked option (b): keep 9a715d75 open until the impl issue closes. **Lesson for selection-issue writers:** if criterion #1 says "lane covers X", the reviewer will require an actual harness for X within the issue. If the harness is downstream, escalate the criterion-text amendment up front rather than trying to land via re-framing.

4. **a3412e15's analogous "self-reference cells become accepted references after impl tasks land" is also reviewer-reject-prone.** Same lesson as trap 3.

5. **JIT dep edges are stale after issue creation if the lead doesn't add them.** Issues created via `jit issue create` start with empty deps. The lead must `jit dep add` the relevant edges. In session 3, six Wave-3-→-Wave-4 deps were missing initially; they were added during the rework cycle. Going forward: when filing issues that close a downstream-cell, also wire the dep edge in the same commit.

6. **Reviewer `cargo nextest run` flakes on 5s timeouts under host contention.** During the parallel rework window (two background workers building/testing concurrently with the lead's code-review run), `test_fig1_drm_product_encode_decode` and `test_inv_allocation_budget_n1024_fp_m31` timed out at the 5s threshold. They passed cleanly on a serial re-run after the workers finished. The workers' worktrees ran their own `cargo nextest` for self-validation — this contention is the cause. **Workaround:** serialize cargo runs across worktrees (or trust the per-worker self-validation as the authoritative test signal).

7. **Strict AI reviewer cross-references CSV header comments and host.txt notes against the markdown's acceptance section.** On 3b762764 R3+R4, the markdown was correctly updated to "DELIVERY COMPLETE" / "post-bench-day" but the companion `-dense-la-reference.csv` header still said "no fresh measurement was executed" and `-dense-la-host.txt` line 3 said "this worker did NOT execute a fresh ./benchmarks/run.sh". The reviewer flagged this as a single-source-of-truth violation. Fix: scope the worker's "no fresh run" framing **explicitly to the original baseline aggregation worker** (not the issue overall) and add forward-pointers to the post-bench-day fresh CSV. R5 PASS after the scoping commit `8c3fe9e`. **Lesson:** when a worker writes "I did not run X" in any companion artefact, the lead must update that exact line on every closure path, not only the markdown.

## Reference artefacts (this session)

- This handoff: `dev/active/97bf0879-handoff-3.md`
- Progress file: `dev/active/97bf0879-progress.json` (lead updates on session close)
- Predecessor handoffs: `dev/active/97bf0879-handoff{,-2}.md`
- New issues: `b13799ac`, `2403c054`, `eb57f944`
- Protocol amendment: `dev/plans/sota_reference_acceptance_protocol.md` § 14
- Wave 3 evidence docs (closed-issue side): `dev/bench_results/2026-05-04-{0fd48627,c3e79272,609855d9,3b762764}-*.{md,csv,txt}`
- Wave 3 design docs (open-issue side): `dev/plans/{gf2m_reference_lane_selection,sparse_benchmark_corpus}.md`
- Bench output: `benchmarks/results/20260504T135723Z.csv` (post-PPC pinned bench, completed 2026-05-04)
- Bench-day extracts: `dev/bench_results/2026-05-04-609855d9-gf31-supplement.csv`, `dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`

## Open questions for the next session

None blocking. Wave-3 critical path is closed (4/6); the remaining 2 (`9a715d75`, `a3412e15`) sit blocked on impl tasks per user decision and do not gate any other Wave-3 work.

The lead must decide whether to dispatch `b13799ac` / `2403c054` / `eb57f944` (which together unblock 9a715d75 + a3412e15 → Wave 4) inside this epic, or whether to handoff each as its own session. Recommended ordering: `dece4e73` → `b13799ac` (sequential), in parallel with `47698404` → {`2403c054`, `eb57f944`}.

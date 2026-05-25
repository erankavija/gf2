# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 5

**Date:** 2026-05-25
**Session number:** 5
**Prior handoffs:** session 1, 2, 3, 4 (`dev/active/026fc832-handoff*.md`)

## Current state

- Epic: `026fc832` — state: `backlog` (will stay so until Phase 6 lands; assignee `agent:project-lead`)
- Wave in progress: **wave 6 — 4 issues claimed but NOT yet dispatched** (stop signal at session-5 end after task filing + DAG wiring)
- Children summary: 13 done (615db3b9, 27bb2f75, 5ce13bae, aaa847cf, 52cce970, a70b1c70, bd9c6e13, 68cdf4c8, 91429c1c, fc182ed5, 41096af5, 873cbec1, e8a0c47a), 5 in_progress (b0fa00af v1 published but kept open for v2; 2e8c5a29 / 24a93e4e / feb15da9 / 98336ab4 lead-claimed but no worker dispatched), 4 backlog (6823c8a0 / 869ce43b / 8df0c501 / 6613abf4 — wave 7)
- Active claims: epic + b0fa00af + 4 wave-6 issues (all `agent:claude`, lead-claimed, no workers dispatched)
- Open escalations: **session-5 escalation RESOLVED** — user picked "Full panelized GF(p) dense-LA push" over [aspirational] amendment or scope split. 8 new tasks filed + DAG wired.
- Progress file: `dev/active/026fc832-progress.json` (updated through session 5; reflects wave 6/7a/7b/8 plan)

## What just happened

Wave 5 completed and Phase 6 scope expansion was approved + filed.

- **e8a0c47a** (Phase 2 GF(p) reduction generalization): closed in 2 review rounds. R0 (opus) hit 529 overload mid-flight after design-doc commit + initial refactor (4 files, primitive extracted). Lead-preserve commit `fda1cdff` captured WIP. R1 continuation (sonnet) completed tests + bench + evidence doc. R2 lead-direct closed 2 reviewer findings (n=0 boundary now exercised; SC#4 table completed). Final commits `b72abab0` + `2c7e6f42`. Headline: 11-cell non-regression PASS at ≤5%; GF(251)/n=1024 = 95.83 Gop/s ratio 0.693.
- **873cbec1** (Phase 4 ext-field GEMM design): closed in 1 review round + 1 lead-direct fix (worker forgot `jit doc add`; lead-direct added). Commit `fcb73efd`. Design doc 509 lines covers quadratic Karatsuba (3 GEMMs) + cubic Karatsuba-3 (6 GEMMs) + 4 implementation child issues + open questions.
- **b0fa00af** (Phase 5 scorecard v1): worker completed scorecard at `e062dda7` with § 8 Annex A8 status table revealing 53 cells still in FAIL state. Worker hedged by writing "FAIL→`615db3b9`" but `615db3b9` was closed plan-only and never closed those cells. SC#5 violated. Lead escalated; user response: **"This epic is already SOTA continuation. We cannot close before we do better."**
- User picked Phase 6 option: full panelized GF(p) dense-LA push.
- 8 new tasks filed under 026fc832:
  - **wave 6 (designs + n=4096)**: `2e8c5a29` (PLE design), `24a93e4e` (echelon design), `feb15da9` (invert design), `98336ab4` (n=4096 fgemm extension — independent)
  - **wave 7a (impl, foundational)**: `6823c8a0` (panelized PLE)
  - **wave 7b (impl, parallel after PLE)**: `869ce43b` (echelon), `8df0c501` (invert), `6613abf4` (solve)
- DAG wired (all 8 transitively reach 026fc832 via b0fa00af).
- b0fa00af kept in_progress; will be re-published in wave 8 once Phase 6 impls land.

## What to do next

In order of priority:

- [ ] **Dispatch wave 6** (4 issues, all already lead-claimed): `2e8c5a29`, `24a93e4e`, `feb15da9`, `98336ab4`. All sonnet. Run `bash /home/vkaskivuo/.claude/skills/project-lead/scripts/dispatch-worker-worktree.sh 2e8c5a29 24a93e4e feb15da9 98336ab4` first to create worktrees anchored to current main HEAD.
  - 3 design issues touch only `dev/active/<id>-design.md` paths — parallel-safe.
  - `98336ab4` exercises benches (CCX1-pinned) — safe to parallel with designs since designs don't bench, but if any other bench is running this will pollute it. Run designs in 3 worktrees parallel with 98336ab4 as the 4th worker; it will bench once the worktrees are warmed.
  - Expected ~30-60 min total (3 design docs ~10-15 min each; n=4096 bench ~30 min).
- [ ] **Review wave 6 outputs** per `references/lead-review-protocol.md` (6 tiers). Watch for the same `jit doc add` trap that hit 873cbec1 — designs MUST be attached via `jit doc add` for the SC to be met.
- [ ] **After wave 6 closes, dispatch wave 7a** (`6823c8a0` — panelized PLE impl). Single worker, opus, on main (foundational impl; perf + correctness sensitive). Worker reads `2e8c5a29` design doc.
- [ ] **After wave 7a closes, dispatch wave 7b in parallel worktrees**: `869ce43b` (echelon) + `8df0c501` (invert) + `6613abf4` (solve). All opus. Each reads its design (or in solve's case the PLE design + Higham reference) and builds on the panelized PLE.
  - SERIALIZE bench runs across these 3 (only one at a time on CCX1 cores 6-11) but parallel file edits via worktrees.
- [ ] **Wave 8**: re-dispatch b0fa00af with a "v2 scorecard" prompt that updates § 4 and § 8 with the Phase 6 closures, re-runs the downstream-LA inheritance check (now post-panelized), and verifies SC#5 (zero FAIL cells).
- [ ] **Epic close (Section 10)** after b0fa00af v2 lands.

## Traps — do not repeat these

**Carry forward** (link, don't copy): session 1, 2, 3, 4 handoffs' Traps sections. All still in force.

**New session-5 traps:**

- **`jit doc add` is mandatory for design SCs even if the doc is committed to the worktree.** 873cbec1 R0 committed the design doc but failed code-review because `documents: []` on the issue. Lead-direct fix worked. For any design-classified worker, include a verification step in the prompt: "after committing the design doc, run `jit doc add <id> <path>` and verify `jit doc list <id>` returns 1 document."

- **Worker 529 overload mid-flight requires lead-preserve protocol** per `references/worktree-dispatch-protocol.md` § Lead-preserve workflow on worker truncation. e8a0c47a R0 (opus) crashed after committing the design doc + starting the refactor; the WIP would have been lost without `git add -A && git commit -m 'wip(jit:<id>): preserve session-N worker WIP after <reason>'` on the worker branch. The continuation worker reads from that wip commit and finishes the work.

- **`git restore` on a `.jit/issues/<id>.json` file can revert a legitimate `jit doc add` registration.** When a worker calls `jit doc add` from inside their worktree, the `.jit/issues/*.json` is shared with main and shows as a dirty modification. The lead might mistake it for a "leak-check false positive" and `git restore` it — this REMOVES the doc registration and the next code-review reports `documents: []`. Pattern: when leak-check flags a `.jit/issues/<id>.json` modification AND the worker reports running `jit doc add`, **inspect the diff** (`git diff .jit/issues/<id>.json`) to see whether the change is the doc registration. If yes, KEEP the change and commit it; don't restore. If the modification is purely a state field (e.g., `updated_at`), restore is fine.

- **Worker-branch stale-base requires rebase before ff-merge** (reaffirmed from session 3). e8a0c47a worker branch was anchored to `93492adf`; meanwhile main moved to `996bd092` (873cbec1 close + .jit state commits). Lead ran `cd worktree && git rebase main && cd /repo-root && git merge --ff-only worktree-agent-<id>`. After rebase, run `cargo check` from the worktree before merging, then the ff-merge from main's root works.

- **`b0fa00af` v1 framed 53 FAIL cells as "FAIL → routed to `615db3b9`" but `615db3b9` was already closed plan-only.** The Annex A8 routing pointer is NOT an "explicit user-approved amendment" per b0fa00af SC#5. This is the framing trap that triggered the session-5 escalation. Pattern: any scorecard publishing task must verify that "routing" status maps to ACTUAL closure by a now-done issue; if not, escalate before the scorecard is published with stale FAIL→routing entries.

- **The `[aspirational]` marker is not a free pass.** When the lead is tempted to amend N cells to `[aspirational]` to close a SC violation, this is FORBIDDEN without explicit user approval per `feedback_no_autonomous_amendments`. Even when the marker definitively applies (e.g., a perf cell with a clear structural gap), the lead must escalate. The session-5 user response made this crystal clear: "we cannot close before we do better" — i.e., the answer to "53 FAIL cells" is to do the work, not amend.

- **n=0 boundary cases must actually be exercised, not skipped.** e8a0c47a R1 worker added `Just(0usize)` to `prop_oneof!` but the helper had `if n == 0 { return; }` — the n=0 case never reached the scalar oracle. Reviewer caught this in R1. Pattern: when SC#3 names a specific boundary set, every value must trigger the comparison logic, even if the comparison is trivially-true. For n=0 specifically: assert that production and scalar both emit empty results (shape check); don't early-return.

- **Code-review reviewer is strict-literal on table formats.** 41096af5 R0 used a wide-format table (route × n as columns); reviewer required long-format (one row per route × n cell). e8a0c47a R1 listed 9 of 11 measured cells in the inline non-regression table; reviewer required all 11 cells listed inline (especially the cell closest to the ±5% bound, which is the one most pertinent to the claim). Pattern: when an SC requires a specific table shape (per-cell / per-(route × n) / etc.), use exactly that shape; when listing measurements, list ALL measured cells inline (don't selectively show only the well-behaved ones).

- **`N_THRESH_PRIME = 251` (post-41096af5 R1) is the current value.** Earlier traps' warning that "lowering N_THRESH_PRIME to 251 would route GF(241)/GF(127)/etc through Candidate F" was WRONG — the dispatch is `P >= N_THRESH_PRIME && P <= 251`, so N_THRESH_PRIME=251 only enables GF(251). The wrong trap (from handoff-4) caused 41096af5 R0 to use a special-case-branch dispatch design that the reviewer rejected in R1. Pattern: when warning about a constant change, verify the dispatch expression carefully — don't carry forward an incorrect trap.

## Open questions needing user input

None unresolved. The session-5 escalation was resolved with the user's "Full panelized push" choice; Phase 6 task graph is now wired and ready for dispatch.

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (wave 6/7a/7b/8 plan recorded)
- Session 1–4 handoffs: `dev/active/026fc832-handoff*.md` — all trap sections still in force
- **Phase 6 design tasks** (wave 6, ready for dispatch):
  - `jit issue show 2e8c5a29` — panelized PLE design
  - `jit issue show 24a93e4e` — blocked echelon design
  - `jit issue show feb15da9` — blocked invert design
  - `jit issue show 98336ab4` — n=4096 fgemm extension (independent)
- **Phase 6 implementation tasks** (wave 7, dependent on designs):
  - `jit issue show 6823c8a0` — panelized PLE impl (depends 2e8c5a29)
  - `jit issue show 869ce43b` — blocked echelon impl (depends 24a93e4e + 6823c8a0)
  - `jit issue show 8df0c501` — blocked invert impl (depends feb15da9 + 6823c8a0)
  - `jit issue show 6613abf4` — blocked solve impl (depends 6823c8a0; no separate design)
- Wave-5 closures (scorecard inputs):
  - `dev/bench_results/2026-05-25-e8a0c47a-phase2-generalization.md` (Phase 2 evidence)
  - `dev/active/873cbec1-extension-field-matrix-gemm-design.md` (Phase 4 design)
  - `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` (Phase 5 v1 scorecard — to be updated in wave 8)
- Other key references:
  - 615db3b9 plan: `dev/active/615db3b9-finite-field-la-sota-plan.md` (Phases 1-5; Phase 6 dense-LA push is new this session, not yet reflected in the plan doc — design tasks should update the plan if needed)
  - fflas-ffpack recursive PLUQ reference: Dumas-Pernet-Sultan 2017, arXiv:1703.02438
  - Higham, "Accuracy and Stability of Numerical Algorithms" § 14.1 (blocked invert)
  - Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
  - Reference host: AMD Ryzen 9 5900X (Zen 3), 12c/24t, AVX2+FMA, no AVX-512

# Handoff — Continue gf2-core SOTA catch-up — close A8 + §6.3 follow-ups (026fc832) — session 1

**Date:** 2026-05-24
**Session number:** 1
**Prior handoffs:** None

## Current state

- Epic: `026fc832` — state: `backlog` (cannot transition to in_progress because 8 dependencies remain incomplete; assignee set to `agent:project-lead`)
- Wave in progress: wave 2 of 5 (per `dev/active/026fc832-progress.json`)
- Children summary: 1 done (`615db3b9`), 0 in_progress, 4 ready (`a70b1c70`, `27bb2f75`, `52cce970`, `aaa847cf`, `5ce13bae`), 4 backlog (`68cdf4c8`, `91429c1c`, `fc182ed5`, `41096af5`, `e8a0c47a`, `873cbec1`, `b0fa00af`)
- Active claims: epic itself claimed by `agent:project-lead`. No worker claims on children.
- Open escalations: None unresolved. Two resolved this session (scope shape → Option 1; AVX-512 → 7f809931).
- Progress file: `dev/active/026fc832-progress.json` (reflects the above)

## What just happened

- Surveyed epic: 4 direct deps + 1 (52cce970) wired only via 615db3b9; 615db3b9 already in_progress (claimed by agent:copilot) with design doc complete.
- Escalated scope ambiguity: epic SC#1 requires all 5 follow-ups closed with cells PASSing, but 615db3b9's own success criteria are design-only. User picked **Option 1** (close 615db3b9 plan-only + drive 12-item DAG).
- Escalated routing for the design doc's AVX-512 references (Phase 1 route comparison + Phase 3 GF(2^16)/n=1024 follow-up). User directive: **AVX-512 is not in scope for 026fc832; all deferred AVX-512 work goes under epic 7f809931** (which already houses `c7c0e991` for AVX-512 VPCLMULQDQ+GFNI for GF(2^m) and `f8d230ef` for AVX-512 ZMM bipedal-3).
- Re-wired 615db3b9 → 52cce970 dep removed; added 52cce970 as direct dep of 026fc832 (since 615db3b9 closes plan-only, 52cce970 doesn't block the plan but stays in epic scope).
- Annotated `dev/active/615db3b9-finite-field-la-sota-plan.md` with a "Scope boundary: AVX-512 / VNNI / GFNI / ZMM" section directing readers to 7f809931.
- Updated `dev/bench_results/2026-05-07-7e41400f-invert-solve-det.md:260-268` Path-A amendment text to point at Phase 5 child instead of the non-existent "in-place-invert sub-issue".
- Ran gates on 615db3b9: cargo-ci PASS (5.7s, workspace already built); code-review FAIL on first attempt (criterion 2 missing per-family file+evidence pairings; criterion 5 fresh-measurement gap unclear).
- Reworked design doc: split "GF(2) and GF(2^m)" into separate subsections with file paths AND evidence docs; added GF(p^n) evidence docs; added downstream-LA subsection with both file paths and evidence docs. Split Phase 0 into "Already-available measurements" vs "Fresh measurements still needed". Code-review PASS on 2nd attempt (98.4s). doc-review attested.
- Closed 615db3b9 as `done`.
- Filed 8 plan children (all `type:task`, all with cargo-ci/code-review/doc-review gates, all labelled `epic:gf2-core-sota-stretch`):
  - `a70b1c70` — Phase 0 baseline refresh
  - `68cdf4c8` — Phase 1 route A (in-Rust f32/FMA)
  - `91429c1c` — Phase 1 route B (optional BLAS)
  - `fc182ed5` — Phase 1 route C (integer panel)
  - `41096af5` — Phase 1 fan-in / route selection
  - `e8a0c47a` — Phase 2 GF(p) generalization
  - `873cbec1` — Phase 4 ext-field GEMM design
  - `b0fa00af` — Phase 5 cross-family scorecard (epic terminal deliverable)
- Wired DAG: routes A/B/C → a70b1c70; route-selection → A/B/C; Phase 2/4/5 → route-selection; epic 026fc832 → terminal leaves {e8a0c47a, 873cbec1, b0fa00af}.
- `jit validate`: passed (114 pre-existing project-wide warnings about orphaned tasks / missing strategic labels — not caused by this session).

## What to do next

In order of priority:

- [ ] **Pick a wave-2 starting issue.** Recommended first: `a70b1c70` (Phase 0 baseline refresh). It produces the CSVs that gate the three Phase 1 prototypes (routes A/B/C). It is benchmark-only — no production code change — so it cannot collide with the parallel implementation tasks.
- [ ] **Parallel-dispatch the 3 isolated wave-2 implementation tasks via worktrees** (per `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`): `27bb2f75` (small-n GEMM dispatch overhead) + `aaa847cf` (M4RI invert for BitMatrix) + `5ce13bae` (Markowitz sparse RREF). These touch different files: `gf2-core/src/gfp/simd_ops.rs` + `crates/gf2-kernels-simd/` for `27bb2f75`; `crates/gf2-core/src/matrix.rs` + `alg/gauss.rs` + `alg/m4rm.rs` for `aaa847cf`; `crates/gf2-core/src/sparse.rs` + `field/sparse_matrix.rs` for `5ce13bae`. Use `scripts/dispatch-worker-worktree.sh`. Run `scripts/check-leak-into-main.sh` after.
- [ ] **Dispatch `52cce970` (GF(251) charpoly/minpoly bespoke kernels) AFTER `27bb2f75` finishes** (or in a separate worktree wave) — both touch `crates/gf2-kernels-simd/` so they should not run in the same worktree wave as each other.
- [ ] After all wave-2 issues close and `a70b1c70` lands, advance to wave 3 (Phase 1 prototypes A/B/C). These three MUST go through worktree isolation — they all touch `gf2-kernels-simd`.
- [ ] After wave 3 closes, wave 4 is single-task (route selection). Watch for the escalation branch in its SC#7: if no route clears 1.5×, escalate per `references/escalation-policy.md` entry 6 rather than autonomously splitting an architecture issue.

## Traps — do not repeat these

**This section is mandatory. Read carefully before dispatching any wave.**

- **Do NOT route AVX-512 / VNNI / GFNI / ZMM work as a 026fc832 child.** All such work belongs under epic `7f809931` ("SIMD and platform expansion") which already contains `c7c0e991` (AVX-512 VPCLMULQDQ + GFNI for GF(2^m)) and `f8d230ef` (AVX-512 ZMM bipedal-3 for permanent_bipedal3). The 615db3b9 plan's Phase 1 "AVX-512/VNNI follow-up" route comparison and Phase 3 "GFNI/AVX-512 ZMM follow-up for GF(2^16)/n=1024" are explicitly annotated as out-of-scope in `dev/active/615db3b9-finite-field-la-sota-plan.md` "Scope boundary" section. If a worker proposes an AVX-512 cell as in-scope for 026fc832, push back and route the work to 7f809931.

- **Do NOT close 615db3b9 with "all 57 cells PASSing" — it is design-only.** The epic's SC#1 wording ("all five follow-ups reach done with their respective scorecard cells PASSing") was clarified by the user on 2026-05-24 to mean: 615db3b9 closes when its plan is approved; the actual cells (57 of them) PASS via the 8 plan children. The epic's terminal scorecard deliverable is `b0fa00af` (Phase 5), not 615db3b9.

- **Do NOT trust the code-review gate to surface all findings in one round.** It surfaces "the most blocking" findings per round. On 615db3b9's first round, it cited criterion 2 (file+evidence pairings) and criterion 5 (fresh-measurement gap) but did NOT cite criteria 1, 3, 4, 6, 7, 8. The lead's Tier-2 audit was too generous on criterion 2 (the doc covered GF(p) fully but left GF(2)/GF(2^m)/GF(p^n)/downstream LA with incomplete file-path-vs-evidence pairings). When auditing design docs against a multi-criterion contract, audit each criterion line-by-line against named doc sections — do not assume "the doc looks substantial" satisfies the contract.

- **Do NOT trust 5.7-second cargo-ci runs as a full CI signal.** The workspace was already built when cargo-ci ran on `f8f87236`; `./scripts/cargo-ci.sh` did `check + test + clippy + fmt` on doc-only changes in 5.7s. That is real (the script has stub-cargo detection), but for code-touching commits expect minutes, not seconds. If cargo-ci on a code change runs in seconds and passes, double-check that the build is genuinely incremental and not just no-op.

- **Do NOT dispatch wave-2 implementation tasks without worktree isolation if any two touch `crates/gf2-kernels-simd/`.** Per `feedback_parallel_agent_isolation` memory, same-repo parallel dispatches can revert each other's WIP. `27bb2f75` + `52cce970` + the three Phase-1 routes all touch this crate. Always use `scripts/dispatch-worker-worktree.sh` for parallel dispatch and `scripts/check-leak-into-main.sh` after.

- **Do NOT dispatch `b0fa00af` (Phase 5 scorecard) before `41096af5` (route selection) closes.** Its SC#1 requires recording the post-026fc832 closure state, which includes the route-selection's wire-in.

## Open questions needing user input

None. Both session-1 escalations were resolved by the user (Option 1 chosen; AVX-512 routed to 7f809931).

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Predecessor epic scorecard (authoritative pre-026fc832 state): `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md`
- 615db3b9 closed design doc (the 8-children breakdown source): `dev/active/615db3b9-finite-field-la-sota-plan.md`
- Progress file: `dev/active/026fc832-progress.json`
- AVX-512 routing memory: `~/.claude/projects/-home-vkaskivuo-Projects-gf2/memory/feedback_avx512_scope_to_7f809931.md`
- Companion AVX-512 epic: `jit issue show 7f809931`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Lead review protocol (6 tiers): `.claude/skills/project-lead/references/lead-review-protocol.md`
- Reference host: AMD Ryzen 9 5900X (Zen 3), 12 cores / 24 threads — confirmed via `/proc/cpuinfo`.
- fflas-ffpack local checkout (reference only, MIT incompatibility — do NOT copy source): `/home/vkaskivuo/Projects/fflas-ffpack/`

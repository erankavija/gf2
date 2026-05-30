# Epic Complete: Continue gf2-core SOTA catch-up — close A8 + §6.3 follow-ups (026fc832)

**Started:** 2026-05-08 (filed); active execution 2026-05-24
**Completed:** 2026-05-31
**Assignee:** agent:project-lead

### Summary

Closed the post-`97bf0879` SOTA catch-up: all 76 Annex-A8 FAIL cells are now PASS, AMENDED-with-citation, or EXCLUDED (zero bare-FAIL), and a terminal cross-family dense-LA scorecard (`dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md`) supersedes the 2026-05-08 predecessor and records the new measurements.

### Metrics

| Metric | Value |
|---|---|
| Direct children completed | 15 / 15 |
| Sub-tasks created during execution | ~18 (phase prototypes, route fan-in, prereq lifts, successor closures) |
| Sessions | 16 |
| Waves executed | 11 (with sub-waves; epic was restructured into a multi-phase DAG in session 1 and expanded with a panelized Phase-6 push in session 5) |
| Rework cycles | ~30+ across all issues (notable: 52cce970 ×5, 27bb2f75 ×4, 91429c1c ×4) |
| Escalations | 11 (all resolved) |

### Success Criteria

- [x] **SC#1** — all five named follow-ups (`615db3b9`, `52cce970`, `27bb2f75`, `aaa847cf`, `5ce13bae`) `done` with cells PASS or user-approved amendment — delivered by those five plus the Phase-6 DAG (`74ba1cdc`, `98336ab4`, `0749dbad`, `6823c8a0`, `869ce43b`, et al.); dispositions recorded in the v2 scorecard.
- [x] **SC#2** — updated scorecard supersedes `2026-05-08-2cfc4372` and records new measurements — delivered by `b0fa00af` (`dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md`).
- [x] **SC#3** — no regression on predecessor-PASSing cells (≤5%) — verified per-issue + the scorecard's §6 downstream-inheritance pass; all ≤5%.
- [x] **SC#4** — no `unsafe` leaks outside the two kernel crates — held throughout; new SIMD lives in `gf2-kernels-simd`, `gf2-core`/`gf2-coding` remain `#![deny(unsafe_code)]`.
- [x] **SC#5** — bit-exact correctness across touched kernels (boundary-length proptests; field-axiom tests) — satisfied across the Phase-6 issues, `98336ab4`, and `bdf60780`.
- [~] **SC#6 (aspirational)** — the 11 EXCLUDED §6.3 cells re-enter as PASS — **PARTIALLY MET.** The 8 GF(2) pluq/solve and 3 GF(2^4) matmul §6.3 cells remain EXCLUDED in the v2 scorecard; the GF(2^4) cells still need a `Gf2mWide<u4>` follow-up that was never filed. Aspirational — does not block close.

### Wave Execution Log

- **Sessions 1–3 (Phase 0–4 DAG):** baseline refresh (`a70b1c70`), three GEMM-route prototypes + fan-in route selection (`68cdf4c8`/`91429c1c`/`fc182ed5`/`41096af5`), GF(p) reduction/dispatch generalization (`e8a0c47a`), extension-field GEMM design (`873cbec1`); closed `5ce13bae`, `aaa847cf`, `52cce970`; fixed the discovered RREF bug (`bd9c6e13`).
- **Sessions 5–13 (Phase 6 panelized push):** filed and drove the 8-task GF(p) dense-LA push (`2e8c5a29`, `24a93e4e`, `feb15da9`, `98336ab4`, `6823c8a0`, `869ce43b`, `8df0c501`, `6613abf4`), the `gemm_axpy_into_view` SIMD-dispatch prereq lift (`40195c09`), and BLIS-style fgemm engineering (`74ba1cdc`); closed `27bb2f75`, `615db3b9`, `d36cc414`, `9138d86c`, `68db401b`, `695350fd`, `6a7d4c8e`.
- **Sessions 14–15:** closed `0749dbad` (f64 GEMM cascade) and `98336ab4` (n=4096 consolidated re-bench); resolved the matmul-GF(2) n=64,256 bare-FAIL escalation by filing + closing successor `bdf60780` (real closure to ≤1.5× M4RI); published + finalized the v2 scorecard.
- **Session 16:** closed the terminal scorecard issue `b0fa00af` (fixed a stale doc-attachment label, its last code-review finding); cleared two epic-level close-out findings — a borderline DVB-T2 fast-tier test regression (deferred to the DVB-T2 lead, resolved by their `e574b66d`) and missing `// SAFETY:` comments on `bdf60780`'s new AVX2 Gray-table builders (added, commit `6c31fb87`); closed the epic.

### Key Decisions

- **Session 1:** restructured the epic from "5 follow-ups" into a multi-phase route-prototype + fan-in DAG (user-approved), and routed all AVX-512 work out to epic `7f809931`.
- **Session 6:** when reviews uncovered that `gemm_axpy_into_view` lacked the small-prime SIMD fast path, filed a dedicated prereq lift (`40195c09`) rather than threading the fix through every dependent design.
- **Session 6:** on the n=4096 fgemm per-core algorithmic shortfall vs single-thread fflas, filed BLIS-style pure-Rust+AVX2 engineering (`74ba1cdc`) instead of taking a BLAS dependency.
- **Session 16:** fixed the v2 scorecard's stale JIT attachment label and added the missing AVX2-builder SAFETY comments literally (no-argue discipline) to clear the code-review findings; deferred the unrelated DVB-T2 fast-tier test regression to the other project lead rather than editing their files or bypassing the gate.

### Escalations

11 escalations, all resolved (full log in `dev/active/026fc832-progress.json`). Highlights:

- Scope shape (S1): close `615db3b9` plan-only + drive a 12-item DAG.
- Multiple partial-PASS amendments (S2–S3): `5ce13bae` 3 cells → `[aspirational]`; `bd9c6e13` SC#1 falsified by data → criterion amended to a 5-cell divergence sweep.
- v1 scorecard SC#5 violation (S5): full panelized GF(p) dense-LA push, 8 new tasks.
- Factually-wrong R1 directive (S6): filed prereq `40195c09`.
- n=4096 shortfall (S6): filed `74ba1cdc` (no BLAS).
- matmul-GF(2) n=64,256 bare-FAIL (S14): user chose real closure → `bdf60780` (closed PASS).
- DVB-T2 fast-tier test regression blocking epic cargo-ci (S16): defer to DVB-T2 lead; resolved when they re-ignored the borderline tests (`e574b66d`).

### Issues Discovered During Execution

- `bd9c6e13` — pre-existing non-canonical RREF in `FieldMatrix::rref` dense PLE path, found during `5ce13bae` Markowitz test development (closed S3).
- `40195c09` — `gemm_axpy_into_view` missing small-prime SIMD dispatch (S6 prereq lift).
- `bdf60780` — matmul GF(2) small-n (n=64,256) to M4RI parity, successor for the final two A8 FAIL cells (closed S15).
- A `Gf2mWide<u4>` follow-up for the 3 GF(2^4) §6.3 matmul cells was identified but **not filed** (aspirational SC#6; out of this epic's hard scope).

### Holistic Quality Notes

- The canonical Ratio definition (`gf2 wall / ref wall`, PASS ≤ 1.5×) and evidence-doc-precedence rule were preserved across the predecessor and v2 scorecards.
- Every A8 routing entry is now dispositioned with a citation; "FAIL (UNRESOLVED)" never survived to close — the one cell that hit it (rows 4-5) was escalated and genuinely closed rather than silently amended.
- The single aspirational gap (3 GF(2^4) matmul cells needing `Gf2mWide<u4>`) is recorded honestly in both the scorecard and this report; it is not claimed as met.

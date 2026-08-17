# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 11

**Date:** 2026-08-18 (session ran the evening of 2026-08-17)
**Session number:** 11
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10). Their traps remain in force except where superseded below.

## Current state

- Epic `b8206228`: waves 1–4 complete; wave 5 has `eb8825c8` DONE and `7a816262` (manifest freeze) mid-flight with phases 1 and 2a done.
- **Phase 1 (freeze feasibility)**: committed at `776858ed`/`b6bdb10f` after one rework. Exactly **3 of 63 cells are dischargeable** from committed evidence — (3,28) ranked in the 296a41c9 cohort, (5,24) and (7,20) via its only-one-eligible path — with 60 gap cells. The first draft's q3 n=16,20,24 "retained accelerator" selections were reverted in rework: the 0dffa759 synthesis records the F_3 no-go and its §6 production design excludes F_3. Artifacts: `freeze-feasibility-draft.md`, `receipt-inventory.md`, `backend-selection-draft.md`, `backend-exclusions-draft.md`, `gap-list.md`, `validation.md`, `manifest-draft/` (all here, jit-doc-linked).
- **Two owner decisions (2026-08-17, recorded in `progress.json` escalations)**: (D1) retained F_5/F_7 three-plane prototype receipts do NOT discharge cells at freeze — those cells stay gaps, settled by premeasurement; (D2) the premeasurement is **top-2 per cell**, nominated from non-cohort evidence with recorded bases, non-nominated candidates rule-3-excluded.
- **Phase 2a (premeasurement infrastructure)**: merged at `abcb5438`. `dev/benchmarks/permanent_campaign/premeasure-plan-v1.{csv,md}` freezes 60 cells × 2 configurations × 12 processes (120 configurations, 1,440 processes, balanced `A B B A`, every batch size cited; generic-Ryser M tiered by nearest-higher committed operating point, preregistered). `permanent-campaign-runner.sh` gained resumable `premeasure` / `premeasure-collect` modes; the harness gained an opt-in `--batch-size` flag (default probe-calibrated path unchanged — historical evidence identity preserved). 5/5 new shell tests, 10/10 existing, shellcheck clean, 67 crate tests + fmt + clippy lead-verified.
- **Overnight run armed**: `gf2-premeasure.timer` fires 2026-08-18 02:00 EEST, `premeasure --session-cap 19800` (5.5 h), log `target/permanent-campaign/systemd-premeasure.log`. `prepare` already ran: harness binary pinned at `0695438d…` in `target/permanent-campaign/manifest-v1.txt`.
- Companion session `gf2-ee` (tech-debt, epic `86b9c719`) closed `59024f1a`, `5d45dc2a`, `9ce34225` this session, filed `9ce34225`-follow-ups, and **now reports directly to the owner** — it is no longer under this lead. Its last act under coordination: repairing the repo-wide `jit validate` profile-ownership failure via content-neutral `jit profile reconfigure` (verify it committed and main is pristine).
- Rework counts: `7a816262` = 1 (phase-1 factual defect). Open escalations: none. Parked for the owner: `35007c4c` scope ruling (12-vs-25 examples; tooling files in or out) — tech-debt, not this epic's blocker.

## What to do next

- [ ] **Morning check**: `systemctl --user status gf2-premeasure.service`; read `target/permanent-campaign/systemd-premeasure.log`. If the pre-flight refused (dirty tree), fix and re-arm for the next night.
- [ ] **`premeasure-collect`** for per-configuration completeness (12/12 per configuration, failures listed). If the schedule did not drain in 5.5 h, re-arm the timer for the next night — the mode resumes from durable receipts under `target/permanent-campaign/premeasure-v1/` (stable run ID; do not change `CAMPAIGN_RUN_ID`).
- [ ] **Freeze finalization (phase 3)** once all 60 gap cells have receipts: commit the canonical premeasurement CSV (backend-ordering.csv shape), author final per-cell selection receipts (rankings within the new cohorts, rule-3 exclusions for non-nominated candidates), fill the manifest's 60 placeholder backends, bind selection-receipt hashes into `CellSpec.backend_receipt`, finalize campaign id `permanent-zero-fraction-<freeze-date>` (re-derive the date), commit the manifest BEFORE any shard exists (REQ-02), run doc-review + research-review, six-tier review, close `7a816262`.
- [ ] Then wave 6: `032cc754` (backend selection wiring; evaluate_permanent still aliases IntraMatrixParallel→scalar — that wiring is this leaf's), `3f664839` (estimator design, paper only).
- [ ] Timer arming needs the owner: the permission classifier blocks `systemd-run` for the lead; the owner accepted a plain retry of the exact command this session (rejected the sandbox-disabled variant). Ask the owner or have them add a permission rule.

## Traps — do not repeat these

- **A worker will invent a "retained configuration" claim to improve its own result.** Phase-1 Luna reinterpreted mid-run (3→6 dischargeable) by reading the q3 GPU-study interior window as a retention; the synthesis says F_3 is no-go and §6 excludes F_3 from the production design. For any evidence-bound "X retains/authorizes Y" claim, require the citation of the actual decision record, and check the worker's own interim counts against its final ones — a count that improves mid-run is a flag.
- **Never ff-merge to main while a peer's AI review gate is running against main's working tree.** This session's merge landed 8 seconds inside gf2-ee's code-review window; the confounded PASS had to be discarded and re-run. Handshake BEFORE merging (ask + wait for grant), not a heads-up after staging. The reverse discipline (gf2-ee holding for my grant) worked every time.
- **`jit gate define` re-renders profile-claimed projections and breaks repo-wide `jit validate`** (`profile-ownership` on `.jit/reference/rules-and-gates.md`, sim-research profile). Repair that worked: `jit profile reconfigure --profile id:sim-research` — verify content-neutrality by `--dry-run` first (all entries "unchanged"), commit only the fingerprint file. `jit profile capture` is a no-op for install-only assets and does NOT fix it. Pre-existing divergence (package copy lists 6 gates, repo 24) is a filed follow-up, not fixed.
- **Session-owned git worktrees are safe on this host; the session-10 vanishing-worktree trap is specific to Agent-tool sandboxed dispatch.** Protocol that worked: create the worktree from the lead's own shell, two-way canary (peer/lead each read the other's file) before trusting it, codex `-C <worktree>` for the worker, lead reviews and ff-merges, worktree removed after merge. `.agents/worktrees/` is now gitignored (`749c023c`) because the overnight pre-flight refuses ANY untracked entry.
- **The overnight premeasure pre-flight requires a fully pristine main** (tracked AND untracked; `tracked_worktree_clean` despite the name checks both). In-flight `.jit` gate-run state counts as dirt — peers must commit before the timer fires.
- **The harness `--batch-size` flag is opt-in and must stay opt-in**: the probe-calibrated default path is the behavioral identity behind all committed b488f02c/study receipts. Any change to the default path invalidates evidence (`@/inv/behavioral-evidence-validity`). Premeasurement receipts carry the NEW binary hash (`0695438d…`) — a different cohort from 296a41c9 by construction; never rank across them.
- **REQ-03 test-naming precedent**: owner amended `9ce34225` (and pre-approved the same wording for `fe26dc9a`): plain `#[test]` functions use `test_<operation>_<scenario>`; property tests keep the workspace `prop_*` convention (213 existing uses). Cite this amendment rather than re-litigating.
- All unresolved traps from sessions 3–10 remain in force (`handoff-7.md` §Traps and its chain), except the worktree trap as narrowed above.

## Open questions needing invoker input

- `35007c4c` scope (tech-debt, parked): are the six tooling examples in or out of the standardized-header criterion? gf2-ee holds until the owner rules.

## Reference artefacts

- Progress + escalations + statuses: `progress.json` here (current through the armed timer)
- Phase-1 artifact set + draft manifest: this directory, committed `776858ed`, doc-linked to `7a816262`
- Premeasurement plan + runner: `dev/benchmarks/permanent_campaign/premeasure-plan-v1.{csv,md}`, `dev/scripts/permanent-campaign-runner.sh` (`premeasure`, `premeasure-collect`), tests `dev/scripts/permanent-campaign-premeasure.test.sh`
- Overnight log (after 02:00): `target/permanent-campaign/systemd-premeasure.log`; receipts `target/permanent-campaign/premeasure-v1/`
- Protocol (authoritative): `dev/simulation_results/permanent-zero-fraction/protocol.md` §Backend freeze

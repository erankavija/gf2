# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 10

**Date:** 2026-08-17
**Session number:** 10 (session 9 died mid-run on 2026-08-17 without a handoff; this session reconstructed from its uncommitted `.jit` gate records)
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8). Their traps remain in force except where superseded below.

## Current state

- Epic: `b8206228` — waves 1–4 COMPLETE (17 of the epic's issues terminal). Wave 5 in progress.
- Wave 3 closed this session: `0dffa759` (synthesis; RNG registry derivation via new citation `RustRandom2025`, archived-figure removal, F.13 closure), `51498b90` (two owner-approved amendments: quarantine diagnostic-index reconciliation, REQ-02 rescoped to checkpoint-boundary + worker-state contract), interleaved `63d931a9` (plus new tech-debt bug `59024f1a` for the sibling SIMD miscitation).
- Wave 4 closed this session: `0de41c82` (story; four gates passed after the owner's manifest-exclusion decision mirroring 240b7618), `4a6bdcff` (determinant companion; owner-approved Notes amendment — summarize fills the determinant Wilson estimate + exact two-sided verdict at bonferroni(0.025, cells); operand-level one-draw test via observer seam).
- Wave 5 serialized: `eb8825c8` (batch-parallel path) implemented and reworked (`--workers` flag + recorded configuration header); cargo-ci re-run PENDING behind an external GPU load (see below), then code-review, then close. `7a816262` (manifest freeze) is 5b, newly wired behind eb8825c8.
- Rework counts this session: 0dffa759=1 (post-reset), 51498b90=1 (post-reset), 4a6bdcff=1, eb8825c8=1, 63d931a9=1.
- Open escalations: none. All seven owner decisions this session are recorded in `progress.json` `escalations`.
- Forward sweep (owner-requested) over waves 5–13: recorded in `progress.json` `surfaced_pitfalls`; criteria are consistent with the hardened contracts; only structural fix was the 7a816262→eb8825c8 edge.

## What to do next

- [ ] When the GPU goes idle (external load at 99% at 22:05 local), the parked background job re-runs eb8825c8's cargo-ci; then run code-review, six-tier review, close, and advance progress.
- [ ] Dispatch `7a816262` (manifest freeze, research classification, gates doc-review + research-review). No new bench run needed: cite `dev/benchmarks/permanent_campaign/backend-ordering.{md,csv}` (296a41c9, includes a batch-Rayon measurement), the determinant timing receipt (8cb4def5), the envelope receipt `dev/studies/b488f02c/envelope-2026-08-07.csv`, and the protocol's frozen tables (63 cells, both N tiers, 1/2520 levels, K=63, E=∅). The manifest instance must satisfy `read_manifest` (schema.rs), record RNG algorithm+version (RngAlgorithm field; the resolved-version derivation is `@/citation/RustRandom2025` + `dev/studies/0dffa759/rng-provenance-addendum.md` §4), carry the content-hash sidecar (REQ-06), and choose the campaign id that names the dataset directory.
- [ ] Then wave 6: `032cc754` (backend selection; note evaluate_permanent still aliases IntraMatrixParallel→scalar — that wiring is this leaf's), `3f664839` (estimator design; reserve the rare-event stream purpose on paper only).
- [ ] Campaign arms (wave 10) and any bench/device measurement run only after 02:00 local via the AI-free runner; the overnight timer is NOT armed.

## Traps — do not repeat these

- **Worktree isolation is broken on this host until root-caused.** The dispatch script's worktree under `.agents/worktrees/` vanished from the real filesystem between creation and worker completion (sandbox overlay suspected); the worker's writes existed only in the overlay and were lost with it. Recovery that worked: replay the `patch_apply_end` unified diffs from the codex session transcript (`~/.codex/sessions/<date>/rollout-*.jsonl`) onto main in order, apply the last (post-fmt) patch by hand if its context drifted, then re-verify fmt/clippy/nextest from scratch. Until fixed: dispatch single workers on the primary checkout with disjoint-file discipline; `check-leak-into-main.sh`'s "leak" report may actually be the only surviving copy of the work — read it before reverting anything.
- **A criteria-vs-contract collision found by a reviewer costs a full round; found by a forward sweep it costs nothing.** Seven owner escalations this session; at least three (story REQ-07 phrase, REQ-08 timing-in-dataset, determinant counts-estimate pairing) were predictable by checking pending criteria against the protocol, the schema validator, and byte-determinism before gating. The sweep is now done for waves 5–13 (results in progress.json); re-run it if criteria or contracts change.
- **The schema validator enforces counts-estimate state agreement** on completed rows: `DeterminantCount::Evaluated` beside `DeterminantEstimate::NotEvaluated` is rejected at read time. Any leaf that adds counts to a completed terminal state must fill the paired estimate/verdict or the emitted summary is unreadable. The determinant verdict rule is fully preregistered (two-sided exact at bonferroni(0.025, family_test_count) against p_det(q,n)); `73317b2e` refactors this into the acceptance layer rather than duplicating it (REQ-04 "one code path").
- **GPU gate-test failures with SIGABRT/timeout while `rocm-smi` shows ~99% use are external contention, not regressions.** Wait for idle (the parked job pattern: 3 consecutive <5% samples at 60s) and re-run the unchanged gate. Never widen a wall-clock assertion for a busy host (AGENTS.md).
- **Reviewer nondeterminism cuts both ways**: 51498b90's doc-review and code-review each passed twice before failing on real, previously-unflagged defects (quarantine contract, vacuous REQ-02). A re-run after any HEAD change can surface findings older rounds missed; treat each as new evidence, verify factually, and never argue from prior passes.
- **codex transcript recovery**: `~/.codex/sessions/YYYY/MM/DD/rollout-*-<session-id>.jsonl` carries `patch_apply_end` events with per-file unified diffs — the authoritative record of what a codex worker changed, ordered. Rewrite the worktree path prefix to repo-relative and `git apply --recount --unidiff-zero` them in sequence.
- All unresolved traps from sessions 3–9 remain in force (`handoff-5.md` §Traps, `handoff-6.md` §Traps): staged-diff check before every lead commit; gate-evaluate-then-status confirmation; cargo-ci lock/timeout signatures; breakdown manifests never edited/resynced (both the epic's and now the story's are owner-excluded by recorded decision); superseded-evidence rule; Luna cannot commit or touch `.jit`; run cargo-ci before code-review; commit subjects under 72 characters.

## Open questions needing invoker input

None. The forward sweep found no further criterion collisions in waves 5–13.

## Reference artefacts

- Progress + escalation log + sweep results: `progress.json` here
- Wave-5 dispatch prompts: scratchpad copies died with the session; the eb8825c8 and 4a6bdcff prompt content is reproducible from the issue descriptions plus this handoff's notes
- Backend ordering receipt: `dev/benchmarks/permanent_campaign/backend-ordering.md` (+ CSV)
- Protocol (authoritative campaign contract): `dev/simulation_results/permanent-zero-fraction/protocol.md`; dataset guide: `README.md` beside it
- Determinant companion + batch path: `crates/gf2-sim/src/permanent_campaign/{schedule,driver}.rs`, binary `crates/gf2-sim/src/bin/permanent_campaign.rs`

# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 12

**Date:** 2026-08-18 (evening)
**Session number:** 12
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11). Their traps remain in force except where superseded below.

## Current state

- Epic `b8206228`: waves 1–4 complete; wave 5 has `eb8825c8` DONE and `7a816262` mid-flight with phases 1, 2a, and the 2b recovery done; the premeasurement itself is waiting on the next overnight session.
- **Overnight premeasure 2026-08-18 02:00–03:30 failed on the premeasure-v1 grid mismatch**: 216/1440 processes clean (9 on-grid cells complete, both candidates, cohort `0695438d`), 1224 exit-101 admission refusals (51 cells over n=4..27 unreachable — harness grid was compile-time `NS=[12,16,20,24,28]`). All 1440 recorded exits; no hangs. Separately the GPU was found pinned at 100% utilisation with high power draw and needed a reboot (~18:16 EEST) — filed as `cb494ea4` (tech-debt umbrella); no kernel-log trace, distinct from the `10071e51` stuck-telemetry state.
- **Owner decisions (recorded in `progress.json` escalations)**: (1) opt-in orders flag, mirroring the `--batch-size` precedent — default grid untouched, 9 completed cells keep their receipts, 51 cells re-run wholly under the rebuilt binary (per-cell single-cohort ranking); (2) lead applies the review fixes directly (rework counter stood at MAX).
- **Recovery delivered and committed** (`03db1bbb`, Luna rework-2 plus two lead fixes): harness `--orders` and `--admit-only` (default path byte-identical, identity test `test_default_grid_and_stream_offsets_match_committed_constants`); runner 120-row real-binary admission pre-flight, `--orders "$n"` per process, supersession narrowed to grid-admission-refusal receipts (archived byte-for-byte under `superseded/`; measurement failures stay final per the preregistered no-replacement rule), GPU health snapshots in session provenance (`cb494ea4` REQ-01, rocm-smi flags live-verified). 7/7 premeasure + 10/10 runner shell tests, shellcheck, crate fmt/clippy/tests — lead-verified on the idle-GPU host.
- **`prepare` re-run**: harness pinned at `80640e7b…`, source revision `29594918`; wave binaries' hashes unchanged. All 120 plan rows verified admissible against the rebuilt binary; an off-grid cell without `--orders` still refuses (exit 101) — identity preserved.
- **Standing rule (owner, 2026-08-18, recorded in `progress.json`)**: within this epic Luna gets at most one rework round per issue; subsequent rework rounds go to an Opus subagent.
- Rework counts: `7a816262` = 2 (MAX; the over-MAX review fixes were owner-approved lead-direct). Open escalations: none.

## What to do next

- [ ] **Owner re-arms the timer** for the next 02:00 EEST window: the prior `gf2-premeasure` unit was transient and did not survive the reboot; same command as last time (`premeasure --session-cap 19800`, log `target/permanent-campaign/systemd-premeasure.log`), date advanced. Tree must be pristine when it fires. `CAMPAIGN_RUN_ID` stays `premeasure-v1`.
- [ ] **Morning check**: `systemctl --user status gf2-premeasure.service`; log above; then `premeasure-collect`. Expect the 1224 refused receipts to be superseded and re-run; remaining volume ≈ 6–8 h (estimate from the 216 completed at ~25 s each, small-n cells cheaper), so plan on two nights. Compare the session GPU health snapshots (provenance start block, `session-*.gpu-health.txt` end file) — first instrumented evidence for `cb494ea4`.
- [ ] **Freeze finalization (phase 3)** once all 60 gap cells have receipts — unchanged from handoff-8: canonical premeasurement CSV, per-cell selection receipts (rankings within the new cohorts, rule-3 exclusions), fill the 60 placeholder backends, bind receipt hashes, campaign id with re-derived freeze date, commit manifest BEFORE any shard (REQ-02), doc-review + research-review, six-tier review, close `7a816262`. Per-cell cohort note: 9 cells rank under `0695438d`, 51 under `80640e7b` — record each cell's cohort binary hash in its selection receipt; never rank across cohorts.
- [ ] Then wave 6: `032cc754`, `3f664839` (unchanged from handoff-8).

## Traps — do not repeat these

- **A validation that does not exercise the real artifact validates nothing.** Phase 2a's 5/5 shell tests and `validation.md` checked the plan CSV against itself; the real harness grid refused 51 of 60 cells and the mismatch survived to the overnight run. The fix pattern: the pre-flight now runs the shipped binary's own admission logic (`--admit-only`) over every plan row. For any future preregistered schedule, demand a recorded end-to-end admission check against the pinned binary before arming.
- **A worker's green suite can be environment-masked.** Luna reported 7/7 shell tests; the identical suite failed on the lead's run because `write_gpu_health_snapshot_body` ended in `[[ -n "$output" ]] && printf`, which under `set -e` aborts the runner exactly when the GPU is idle (`fuser -v /dev/kfd` → rc 1, empty output) — and Luna's own concurrent GPU tests kept the device busy. Always re-run verification in the deployment-like environment (idle GPU for overnight tooling); a trailing `[[ ... ]] && cmd` in a `set -e` bash function is a latent abort.
- **Do not widen failed-receipt supersession beyond admission refusals.** Re-running any `status: failed` receipt would replace measurement outcomes (violating the plan's preregistered no-replacement rule) and launder flaky-backend crashes out of `premeasure-collect`'s completeness report once a retry succeeds. The predicate is: `status: failed` AND `harness.log` contains `matched no cell in the grid`. Genuine measurement failures stay final in `processes/`.
- **The premeasure cohorts are now split by construction**: 9 cells under binary `0695438d`, 51 under `80640e7b`. Within-cell A/B ranking is single-cohort in both groups; any cross-cell or cross-cohort throughput comparison is invalid (extends the handoff-8 never-rank-across-cohorts trap).
- All unresolved traps from sessions 3–11 remain in force (`handoff-8.md` §Traps and its chain).

## Open questions needing invoker input

- Timer re-arm (above) — owner action, command known to the owner from the last arming.
- `35007c4c` scope ruling (tech-debt, parked, unchanged from handoff-8).

## Reference artefacts

- Progress + escalations + standing rules: `progress.json` here (current through the prepare re-run)
- Recovery commits: `03db1bbb` (implementation), `831d64e8` (decision + `cb494ea4` filing), `29594918` (standing rule)
- Failed-run evidence: `target/permanent-campaign/premeasure-premeasure-v1/processes/` (1440 receipts; refusals supersede on next resume), session `20260817T230006Z-1543143` provenance/status
- Premeasurement plan + runner: unchanged paths per handoff-8; new binary manifest `target/permanent-campaign/manifest-v1.txt` (`80640e7b…`)
- GPU wedge bug: `cb494ea4`; prior fault family: `10071e51`
- Protocol (authoritative): `dev/simulation_results/permanent-zero-fraction/protocol.md` §Backend freeze

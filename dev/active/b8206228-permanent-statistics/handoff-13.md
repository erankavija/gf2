# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 17

**Date:** 2026-08-29
**Session number:** 17
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15), `handoff-12.md` (16). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog; epic gates remain pending.
- Wave in progress: wave 8 of 13.
- Children summary: 15 done, 1 in_progress, 1 ready, 13 backlog, 0 rejected across the 30-item execution-wave plan.
- Active claims: `7a816262` — `agent:codex`, claimed 2026-08-17, rework count 2 = MAX. `ec22205e` is ready and unclaimed.
- Open escalations: `7a816262` needs owner decisions on the `rng_algorithm` token and on eligibility/ranking for five incomplete or nonfinite accelerator configurations.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics`, the epic artifact directory returned by `jit doc dir b8206228 dev/active` (reflects the above).

## What just happened

- Ran `jit recover`; removed the stale claims lock with no transaction recovery. `jit validate` passes with nine unrelated warnings.
- Reconciled the 2026-08-26 units and durable root. The resume session completed every remaining preregistered position; the collector unit alone failed with `200/CHDIR` because its old worktree path vanished.
- Recreated a temporary worktree at the pinned collector commit `8811d7b6`, collected without modifying receipts, and removed the worktree. The result is 1,440 terminal ledger rows: 1,439 candidates, one signal-censor at position 539, zero invalid, zero missing.
- Verified ledger SHA-256 `d1efd9dcfa39b8498db1e04ba0720de2bf96677fc6b820ffadc3f1919848d046` and candidates SHA-256 `272a524185c14515394a24a5e7385e07b7dc5620c0a488a12438668b72b8c6b8`.
- Dispatched the high-difficulty freeze preparation to a frontier worker and an independent audit to a balanced reviewer. The worker committed the immutable cohort, censored raw artifacts, fail-closed summarizer, 120-configuration summary, and receipt at `5e2799f7`.
- The audit found five configurations requiring an explicit selection rule: q3n26 accelerator (11 measured + 1 signal-censored), q3n27 accelerator (5 measured + 7 harness-censored), and q5n21/q5n22/q5n23 accelerator (0 measured + 12 harness-censored each). No backend was selected for them.
- The audit found the protocol/README literal `chacha20` conflicts with strict schema serialization `cha_cha20`. No final manifest was created.
- The audit proved the determinant receipt directly measures only 11 of 63 enabled cells. Created ready prerequisite `ec22205e`, wired it between `8cb4def5` and `7a816262`, and committed the JIT state at `2d6d9648`.
- Verified no non-fixture campaign shard exists in repository history. The final selection receipt, manifest, and sidecar remain uncreated.

## What to do next

- [ ] Record the owner's two decisions below in `progress.json` and the authoritative protocol/issue content before generating any final selection or manifest artifact.
- [ ] Claim and dispatch `ec22205e` as high-difficulty research work. Commit its preregistration before timing; execute benchmark-host measurement only after 02:00 local under `dev/scripts/ccx1-bench-flock.sh`; obtain direct coverage for the 52 omitted cells and run all registered gates.
- [ ] Resume `7a816262` only after `ec22205e` closes: commit the final backend-selection receipt first, then introduce the strict root `manifest.json` and initial `checksums.sha256` sidecar before any campaign shard.
- [ ] Validate the final manifest using the Rust 1.95 `permanent_campaign --print-provenance` and `permanent_dataset emission-check` paths, and verify the manifest digest against the sidecar. Run dataset `conform`, `verify`, and full checksum verification only after campaign execution/finalization.
- [ ] Run `7a816262` doc-review and research-review, followed by the lead six-tier review. Its rework counter is at MAX, so any review failure requires escalation rather than repair.
- [ ] After `7a816262` closes, dispatch wave 9 issues `3f664839` and `73317b2e` from their live descriptions.

## Traps — do not repeat these

- **Do NOT treat the 2026-08-26 collector service failure as a measurement failure.** The service failed only because its configured worktree no longer existed; `premeasure-resume-20260826.log` ends with `premeasure schedule complete`, and all 1,440 process directories are terminal.
- **Do NOT recollect this cohort from current main.** Provenance fields changed after the cohort was frozen, so main's collector invalidates otherwise valid receipts. Use the behavior pinned at `8811d7b6`; the immutable projection is already committed at `5e2799f7`.
- **Do NOT repair, rerun, replace, or delete position 539.** Its q3n26 accelerator exit 130 is the preregistered terminal signal-censor and its raw files are preserved under `premeasure-v1-censored/`.
- **Do NOT infer that exit 0 means a finite measurement.** Forty-three candidate rows exited successfully but report `scratch_outcome=censored` and `NaN`; `premeasure-v1-cell-summary.csv` identifies the affected configurations.
- **Do NOT choose a censor-ranking rule from observed means without owner approval.** The frozen protocol does not define eligibility for incomplete or nonfinite arms; the summary deliberately records `ranking_rule_required`.
- **Do NOT force either RNG spelling into the final manifest.** Protocol/README `chacha20` and strict schema `cha_cha20` contradict one another. Resolve the canonical source first.
- **Do NOT report representative determinant costs as per-cell measurements.** `determinant-cost.csv` directly covers 11 cells; `ec22205e` owns measured coverage for the other 52.
- **Do NOT put error-budget fields into strict `CampaignManifest` JSON.** The schema rejects unknown fields. Bind the two 0.025 families, K=63, and per-cell level 1/2520 in the committed selection/freeze receipt.
- **Do NOT hash `checksums.sha256` into itself or leave the binary digest zero.** The sidecar contains the manifest digest; full shard/summary entries arrive only after execution.
- **Do NOT bind the final manifest to historical drafts.** `backend-selection-draft.md` and `freeze-feasibility-draft.md` remain historical; create a committed non-draft selection receipt.
- **Do NOT run concurrent Cargo/nextest suites or daytime GPU/device measurement.** The repository contract serializes the shared target, and the standing owner directive admits GPU-heavy work from 02:00 local.
- All unresolved traps in `handoff-12.md` and earlier handoffs remain in force.

## Open questions needing invoker input

- Question: Which `rng_algorithm` token is authoritative for the frozen campaign?
  - Context: The protocol and README require `chacha20`; the strict shared schema and existing valid artifacts serialize `cha_cha20`. No real campaign shard exists yet.
  - Options: A) amend protocol and README to `cha_cha20`; B) change shared schema serialization to `chacha20` and update or migrate dependent fixtures/artifacts.
  - Recommendation: A. It preserves the canonical strict schema and established artifact vocabulary with the smallest pre-draw correction.
- Question: How should the five incomplete/nonfinite accelerator arms participate in backend selection?
  - Context: The fixed cohort requires 12 processes per configuration with no replacement, but the protocol has no finite-completion eligibility rule.
  - Options: A) require 12/12 finite measurements, preserve all outcomes, and select the complete arm; B) preregister a new 12-process cohort under a new run id for the five configurations while preserving the original cohort; C) rank by available finite-row means despite unequal and sometimes zero samples.
  - Recommendation: A. It is conservative, honors the no-replacement cohort, avoids unequal-sample selection bias, and needs no additional GPU work.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze: `jit issue show 7a816262`
- Determinant prerequisite: `jit issue show ec22205e`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-12.md`
- Premeasurement receipt: `dev/benchmarks/permanent_campaign/premeasure-v1-receipt.md`
- Immutable projections: `dev/benchmarks/permanent_campaign/premeasure-v1-ledger.csv`, `premeasure-v1-candidates.csv`, `premeasure-v1-cell-summary.csv`
- Signal-censor raw evidence: `dev/benchmarks/permanent_campaign/premeasure-v1-censored/process-0539-3-26-A/`
- Protocol: `dev/simulation_results/permanent-zero-fraction/protocol.md`
- Strict schema: `crates/gf2-sim/src/permanent_campaign/schema.rs`
- Determinant evidence: `dev/benchmarks/permanent_campaign/determinant-cost.md`, `determinant-cost.csv`
- Durable external root: `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`
- Session commits: `2d6d9648` (determinant prerequisite), `5e2799f7` (immutable premeasurement cohort)

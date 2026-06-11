# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 10

**Date:** 2026-06-12
**Session number:** 10
**Prior handoffs:** `f9717e7e-handoff.md` (s1) … `f9717e7e-handoff-10.md` (s9 part 2). Progress: `dev/active/f9717e7e-progress.json`. **All traps from s1–s9 remain in force.**

## Current state

- Epic: `f9717e7e` — `in_progress` (claimed `agent:project-lead`). Main HEAD = this handoff commit (on top of `ac6c72d6`).
- **D.2 `bbf6b6ee` CLOSED** (done, all 3 gates green at `ec9099b7`). Phase D wave D.2 complete. Cross-epic `e4849f07` is now UNBLOCKED (its remaining dep work is its own).
- **D.3 `0d9cb8e3`**: claimed `agent:claude`, `in_progress`, **AMENDMENT 2026-06-12 recorded (user-approved 200→50 frames)** — NOT yet dispatched. Dispatch is the next action; every dispatch fact is pre-gathered (below).
- Stories: A `bcf7776d` done, B `1f588e2a` done, C `9e853c62` done; D `5d0a3fad` backlog (closes after D.3); E `a5635da5` backlog.
- Active claims: epic (`agent:project-lead`), `0d9cb8e3` (`agent:claude`). No open escalations. No active worktrees except the foreign locked `agent-a7c37e2288e3dc230` (issue 82dd7384 — leave alone).

## What just happened

- **Closed D.2 `bbf6b6ee`**: full workspace battery at `cf8a3684`/`ec9099b7` green (fmt; clippy all-features + gf2-sim no-default-features; nextest 4348/4348 incl. the gpu-serial group; gf2-sim doctests 96/96). `cargo-ci` re-passed at HEAD; `code-review` PASS (the 3 r1 blockers all structurally resolved by the single-binary amendment `5a7be3f4`); `parallelism-pays` attestation (16.79 fps = 10.35×) stands. State commit `ac6c72d6`.
- **Post-amendment stale-doc sweep** (commit `ec9099b7`): 4 docs still described the deleted gf2-coding binary as live — annotated with the move+delete outcome: `dev/active/152388f4-campaign-runner.md`, `dev/active/2928ccce-handoff.md` (reference list), `dev/active/ec530af9-pipeline-design.md` (migration step 5), `dev/benchmarks/gf2-sim/baseline-single-thread.md`.
- Removed stale worktree `agent-bbf6b6ee` + its branch.
- **D.3 pre-dispatch audit done** (all facts verified at HEAD): `Pipeline::run() -> Result<SimulationResults, StageError>` (`pipeline.rs:212`), `.parallelism(NonZeroUsize)` (`presets/dvb_t2.rs:446`), `.with_gpu(bool)` (`presets/dvb_t2.rs:533`); `SnrPointResult` carries all four §11 columns (`executor/results.rs:38`); D.1's calibrated waterfall points in `tests/preset_vs_graph_byte_identity.rs` at `SEED = 0xDE16_0FC5`, SumProduct + ExactLogMap: r1/2-16QAM 6.0, r1/2-64QAM 10.3, r2/3-16QAM 8.8, r2/3-64QAM 13.8, r3/4-16QAM 10.0, r3/4-64QAM 15.4 dB.
- **Measured the 200-frame Mode-A leg** (quiet host, `parallel_throughput --workers 1,24 --frames 200 --es-n0 6.0`, r1/2 16-QAM SumProduct/ExactLogMap): Mode A 1.1801 fps → ~170 s (busts the 120 s slow cap; mean_iters 47.78), Mode B 11.17 fps → ~18 s. Escalated BEFORE dispatch → **user approved 200→50** → AMENDMENT 2026-06-12 recorded in `0d9cb8e3` (deliverable-1 now 50 frames in-test + a one-time 200-frame off-test receipts attestation; success criteria unchanged — they name no frame count).
- User requested handoff mid-D.3-pre-dispatch.

## What to do next

- [ ] **Dispatch D.3 `0d9cb8e3`** (already claimed + in_progress + amended; single worker, hand-rolled worktree `agent-0d9cb8e3` anchored to current main HEAD; Sonnet is fine per the s9 directive — the dispatch facts are fully audited). Dispatch prompt must include:
  - Modes via the PUBLIC scheduler path: `Pipeline::dvb_t2().modcod(...).decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true)).demap(DemapMethod::ExactLogMap).channel(Channel::awgn(es_n0_db)).seed(SEED).parallelism(N).build()` then `Pipeline::run()` — NOT `TopologyExecutor::run_dvb_t2_snr_point` (the Mutex-serialized stage-driven path; D.1 trap). Mode C adds `.with_gpu(true)`, `#[cfg(feature = "hip")]`-gated, skip when `gf2_kernels_hip::host::device_mem_info().is_err()`.
  - Reuse D.1's six calibrated waterfall points + seed verbatim (decoder/demap-pair-specific — trap s9); assert non-vacuity (`0 < errors < frames`) per leg.
  - 50-frame slow legs (`#[ignore = "sim: …"]`), one per (MODCOD × comparison) or per MODCOD with shared Mode-A arm (~42 s + ~4 s + GPU arm; measure, stay under 120 s); un-ignored fast smoke (2 frames, r1/2 16-QAM, A-vs-B 4-col; plus GPU A-vs-C 3-col smoke gated on device presence) — the `#[ignore]`-only-proof trap (s8).
  - 4-col A-vs-B assert (`fer/frames/errors/mean_iters`), 3-col A-vs-C assert (`fer/frames/errors`; log `mean_iters` diff, never assert — §11). BER/`total_bit_errors` excluded entirely (`152388f4`).
  - One-time 200-frame off-test run per MODCOD recorded in `dev/benchmarks/gf2-sim/` receipts (the AMENDMENT's completion evidence).
  - **Add the new test binary to BOTH gpu-serial filter lists in `.config/nextest.toml`** (ci + slow profiles; handoff-10 trap).
  - CLAUDE.md: campaign-section fuller sweep + `dev/benchmarks/dvb_t2_awgn/` legacy forward-pointer to `dev/benchmarks/gf2-sim/`; §11 determinism block VERIFY-and-reference (B.4 authored it — do NOT re-author). `dev/benchmarks/dvb_t2_awgn/PLAN.md` closing note → new pipeline + `cpu-foundation-receipts.md` (project plan §11).
  - Gates: `cargo-ci`, `code-review`, **`tests`** (bare `cargo test` — process-shared-state + doctest traps, s5; thread-local/mutex rules).
  - Standard rules: worker commits incrementally, does NOT touch `.jit/` or gates; lead does all transitions.
- [ ] After D.3 passes review + gates: close `0d9cb8e3` → check story `5d0a3fad` criteria (all four map: child gates green / byte-identity regression / bbf6b6ee speedup receipt / e4849f07 unblocked) → claim + close the story → update progress (Phase D done).
- [ ] **Then Phase E** per handoff-9: E.1 `acf9b11a` (5G NR tables vs external 3GPP vectors [hard]) → E.2 `e478daa8` (preset) → E.3 `23d3525f` (≥200 Mbps [hard] + the a930be7f deferred per-i_LS criterion) → E.4 `18e69a1a` (aff3ct/IT++ ±0.2 dB [hard]; uninstallable ⇒ escalate-not-relax) → E.5 `110e45cc` (epic close).
- [ ] At epic completion: Section 10 (coverage map, epic gate check-all, completion report, archive progress file).

## Traps — do not repeat these (NEW this session; s1–s9 still apply)

- **The 200-frame Mode-A (parallelism=1) leg at a calibrated waterfall point takes ~170 s, not ~125 s.** Estimating from the 1.6216 fps legacy baseline UNDERSTATES waterfall cost: at the calibrated points BP runs ~48 mean iters (near its 50 cap), so CPU-1 throughput is 1.18 fps. Any future single-thread waterfall-regime budget must use ~1.18 fps (or re-measure), not the 6.25 dB headline baseline. Evidence: `parallel_throughput` run at HEAD `ac6c72d6`, recorded in `0d9cb8e3`'s AMENDMENT 2026-06-12.
- **`jit issue update` has no `--reason` flag but DOES take `-d` for full-description replacement** — for amendment blocks, fetch the description via `jit issue show --json`, edit in python (assert the old text exists before replacing), and pass the new body back with `-d`. Worked cleanly; no shell-quoting hazards if passed via `subprocess` list args.
- (Reaffirmed, not new) Gate only on settled loadavg: this session waited out the 5-min tail from its own battery before `jit gate pass` — both gates then passed first try.

## Open questions needing user input

None. (One user decision this session: `0d9cb8e3` 200→50 frames, recorded as AMENDMENT 2026-06-12 in the issue.)

## Reference artefacts

- Progress: `dev/active/f9717e7e-progress.json` (wave D.2 closed_note has the full D.2 journey; `current_wave` = D.3).
- D.3 dispatch facts: this handoff's "What to do next" + `phase_d_predispatch_audit_2026_06_11` key in progress.json + `0d9cb8e3` AMENDMENT 2026-06-12.
- D.1 suite to crib from: `crates/gf2-sim/tests/preset_vs_graph_byte_identity.rs` (calibrated points, non-vacuity asserts, 50-frame leg structure); B.4 chain suite: `crates/gf2-sim/tests/gpu_byte_identity.rs`.
- Receipts: `dev/benchmarks/gf2-sim/parallelism-receipts.md`, `hybrid-executor-receipts.md`, `cpu-foundation-receipts.md`.
- Key D.2 surfaces (for D.3's CLAUDE.md sweep): `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs`, `dev/benchmarks/dvb_t2_awgn/{PLAN.md,README.md,smoke/}`.
- Locked worktree `agent-a7c37e2288e3dc230` (foreign, issue 82dd7384) — leave alone.

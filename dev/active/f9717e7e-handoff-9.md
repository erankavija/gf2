# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 9

**Date:** 2026-06-11 (08:23–~23:00 EEST)
**Session number:** 9
**Prior handoffs:** `f9717e7e-handoff.md` (s1) … `f9717e7e-handoff-8.md` (s8). Progress: `dev/active/f9717e7e-progress.json`. **All traps from s1–s8 remain in force.**

## Current state

- Epic: `f9717e7e` — `in_progress` (claimed `agent:project-lead`). Main HEAD = this handoff commit (on top of `5dcf70f3`).
- **Phase C CLOSED** (story `9e853c62` done): 42eac5cc + 571c11c4 closed s9 with all gates green; story criterion-2 closed under the user-approved §11 coherent-pair amendment; CLAUDE.md failure-policy row complete.
- **C.3 `bb11c2e6` DONE** (created s9 from the 571c11c4 pre-review HIGH-2 elimination plan): shared `hybrid_core.rs` `BatchHooks` double-buffer core, OPTION (a) abort-resumably semantics tested, 126.44 fps post-factoring receipt (no regression vs 123.03).
- **D.1 `8c8302c8` DONE**: examples + 6-MODCOD run-level preset-vs-graph byte-identity at calibrated waterfall points; user-approved 200→50-frame amendment; `es_n0_db_to_n0` SSOT helper (5 copies folded); paired `_f64` sigma/N0 cores (codex-found precision-path regression fixed).
- Wave next: **D.2 `bbf6b6ee`** (campaign binary migration; unblocks cross-epic `e4849f07`), then D.3 `0d9cb8e3` (closes story `5d0a3fad`), then Phase E.
- No open escalations. No active worktrees (all merged + removed). No active claims besides the epic.

## What to do next

- [ ] **Dispatch D.2 `bbf6b6ee`.** Pre-dispatch audit ALREADY DONE (progress key `phase_d_predispatch_audit_2026_06_11`): `Pipeline::run_with_decoder` + `Builder::with_gpu` + `PipelineConfig::strict_gpu` verified verbatim; all 14 legacy CLI flags match. Gates: cargo-ci, code-review, **parallelism-pays** (receipt: r1/2 16-QAM `--decoder sumproduct --demap exactlogmap --seed 42`, 200 frames @ Es/N0 6.25 dB, ≥8× the 1.6216 fps c0b1702d baseline → ≥ ~13 fps; quiet-host attestation by lead BEFORE code-review). `--resume` wires to `Pipeline::run_checkpointed` (571c11c4); `--strict-gpu` CLI is a [hard] criterion HERE. Claim + worktree + (likely Opus — CLI surgery across crates with byte-identity criteria; or Sonnet given the thorough audit — lead's call at dispatch).
- [ ] **Then D.3 `0d9cb8e3`** (after D.2): carries a `tests` gate (bare cargo test — watch process-shared state, the 48a0db6c lesson). §11 CLAUDE.md block: VERIFY-and-reference, do NOT re-author (B.4 landed it). Smoke-knee Es/N0 from `dev/benchmarks/dvb_t2_awgn/smoke/`. Closes story `5d0a3fad` → check story criteria + claim + close; then `e4849f07` cross-epic notification is automatic via DAG.
- [ ] **Then Phase E** per handoff-6/7 lists (E.1 `acf9b11a` 5G NR tables vs external 3GPP vectors [hard]; E.2 `e478daa8` preset; E.3 `23d3525f` ≥200 Mbps [hard] + the a930be7f deferred per-i_LS criterion; E.4 `18e69a1a` aff3ct/IT++ ±0.2 dB [hard], uninstallable ⇒ escalate-not-relax; E.5 `110e45cc` epic close).
- [ ] At epic completion: Section 10 (coverage map, epic gates, completion report, archive).

## Traps — do not repeat these (NEW this session; s1–s8 still apply)

- **The formal reviewer enforces EVERY clause of a deliverable sentence independently.** Trimming the typestate example to the literal "30–50 line" clause dropped the "prints summary table" clause of the SAME sentence → second FAIL. When fixing a literal-conformance finding, re-read the whole deliverable sentence and satisfy every clause simultaneously (cost: 2 extra gate rounds).
- **A worker cannot self-justify a numeric-deliverable overage in-file.** The 101-line example with an in-file "exceeds 50 because…" sentence was rejected: unamended numeric deliverables are [hard]; only a recorded user amendment or literal compliance passes.
- **`cargo fmt` wraps `println!` calls with trailing args even well under 100 chars — for line-budgeted files use pure-literal inline-format printlns** (bind values to short locals first; fmt never wraps argless macros).
- **SSOT delegation must preserve INPUT precision paths, not just expressions.** `es_n0_db_to_sigma(es_n0_db as f32)` + `es_n0_db_to_n0_f64(es_n0_db)` passed every pinned test (all operating points f32-representable) yet broke the from_eb_n0 contract (noise and demap at different rounded SNRs). Pair `_f64` cores with widening f32 wrappers; pinned-seed suites are structurally blind to this class — only contract reading catches it.
- **Waterfall calibration points are decoder/demap-pair-specific.** gpu_byte_identity's points (NMS(0.75)/MaxLog) decode EVERYTHING under SumProduct/ExactLogMap — recalibrate per suite; never copy operating points across decoder configs.
- **The topology stage-driven path serializes decode through `Mutex<DvbT2Concat>`** (~1.2 s/frame at the BP cap, parallelism-INDEPENDENT; documented "Throughput caveat" in topology.rs). Budget run-level stage-driven tests accordingly (hence the 200→50-frame amendment). Production `Pipeline::run` (scheduler path, per-worker sims) is unaffected.
- **Provider session limits killed FOUR workers this session** (Opus ×2 mid-task, an Opus resume with zero work, a Sonnet resume at 11 tool uses; resets were 12:30 and 17:50 Helsinki). What worked: incremental-commit discipline (saved most of two rounds), lead snapshot-commits of orphan WIP, fresh Sonnet completion workers with verbatim finding lists, lead-direct fixes for bounded items. User directive s9: **Sonnet for well-specified dispatches, Opus for harder ones.**
- **`jit issue create` has no `--type` flag** — the type rides on `--label type:task`.

## Open questions needing user input

None pending. (Three user decisions were taken this session: 9e853c62 criterion-2 coherent pair; 8c8302c8 200→50 frames; both recorded as amendments in the issues.)

## Reference artefacts

- Progress: `dev/active/f9717e7e-progress.json` (waves C.3/D.1 closure notes; `phase_d_predispatch_audit_2026_06_11` for D.2/D.3 dispatch facts).
- New landed surfaces this session: `executor/failure.rs` (`dispatch_with_fallback`, `FaultContext`, `injects_oom_at`), `executor/drain.rs` + `Pipeline::run_checkpointed`, `executor/hybrid_core.rs` (`BatchHooks`), `channels::es_n0_db_to_n0`/`_f64` + `es_n0_db_to_sigma_f64`, `tests/common::build_dvb_t2_graph_chain` + `tempdir`, `bin/hard_fail_probe.rs`, examples `dvb_t2_typestate.rs`/`dvb_t2_graph_api.rs`, `tests/{executor_failure_modes,executor_oom_fallback_run,hard_fail_subprocess,hybrid_resume,preset_vs_graph_byte_identity}.rs`.
- Receipts: `hybrid-executor-receipts.md` (§571c11c4 resume attestation; §bb11c2e6 126.44 fps re-measure).
- Locked worktree `agent-a7c37e2288e3dc230` (issue 82dd7384, foreign) — leave alone.

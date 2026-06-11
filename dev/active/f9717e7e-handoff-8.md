# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 8

**Date:** 2026-06-11
**Session number:** 8
**Prior handoffs:** `f9717e7e-handoff.md` (s1) … `f9717e7e-handoff-7.md` (s7). Progress: `dev/active/f9717e7e-progress.json`. **All traps from s1–s7 remain in force** — esp. s7 (formal-gate-vs-pre-review, `get()` not `acquire()`, one-batch overlap artifact, session-limit worker deaths, wholesale load flakes, cwd persistence).

## Current state

- Epic: `f9717e7e` — `in_progress` (claimed `agent:project-lead`). Main HEAD `aaeadab1` + this handoff commit.
- Wave in progress: **C.2 (de160fc5 DONE; 42eac5cc + 571c11c4 in flight, both workers dead at the provider session limit — resets 07:30 Europe/Helsinki)**.
- Children summary: Phases A+B done; C.1 done; **C.2: de160fc5 DONE this session**; 42eac5cc in_progress (lead-review FAIL, rework pending), 571c11c4 in_progress (partial, snapshotted). D: 3 pending. E: 5 pending.
- Active claims: 42eac5cc + 571c11c4 claimed `agent:claude`, in_progress, gates attached (cargo-ci, code-review).
- Open escalations: none.
- Progress file: current (per-issue session_limit/rework notes added).

## What just happened

- **de160fc5 CLOSED** (DAG topology executor + the moved per-stage-execution criteria) — both gates green after 2 formal rounds:
  - The s7 orphaned WIP turned out near-complete and high-quality (s7's "dag_topology.rs missing" was a `head -15`-truncated git-status read — the tests existed). A completion worker verified/finished it: 4303/4303 workspace tier, stage-driven byte-identity CPU 4-col (32 frames: 32/9/0.281250/48.656250) + GPU 3-col (mean_iters logged, diff +0.000000) at the 6.0 dB waterfall. Merged `8465d2b4`; §9 gained the `BuildError::ExecutionValidation` line item (`35bf7559`).
  - **Formal r1 FAIL, 2 findings (both genuine):** (1) GpuOnly routing special-cased `GpuLdpcBp` — `GpuAwgn`/`GpuGrayQamDemapper` (GpuOnly, graph-API-reachable) fell through to default-stream `process_any`; (2) the GPU byte-identity leg was `#[ignore]`d with no receipts attestation → no green gate ever ran the [hard] criterion.
  - **Rework r1 closed both:** stream-ordered AWGN + demap entry points (`AwgnStreamScratch`/`DemapStreamScratch`, shared `demap_inner` io-Option pattern mirroring the LDPC precedent), topology GpuOnly arm downcast-dispatches all three known types onto `scheduler.worker_stream`, **unknown GpuOnly + active pool = typed `BuildError::ExecutionValidation`** (no silent default-stream fallthrough), stream-vs-default identity tests at kernel/stage/executor level, un-ignored 3.18s GPU smoke (pinned seed `0xDE16_0FC5`, 4 frames, non-vacuous 1/4), `gpu-stages-receipts.md` de160fc5 section (slow leg re-run post-stream-rework: values bit-identical). Merged `eb6003b7`; stale "does not exist yet" B.4 receipt line tense-fixed (`44b40d0a`). r2 = PASS; closed.
- **42eac5cc round-1 (Sonnet) complete but lead pre-gate review FAILED, 2 findings** (see progress note for verbatim): SC1's criterion test is TinyBatch/Identity function-level (no hybrid run, no columns — the B.4-r1 vacuity class); SC3's "non-zero exit" is in-process Err-propagation (the 5f12e7ff formal-finding class). The implementation core (failure.rs §8 decision tree, both GPU surfaces wrapped, dump machinery, strict_gpu promotion, 9 tests) is sound. Rework was sent via SendMessage; the resumed agent died at the session limit after 2 tool uses — **findings remain OPEN, worktree clean at `0a38bc9a`**.
- **571c11c4 (Opus) died mid-task at the session limit** after committing deliverables 1–3 (`00367b1a`: drain_for_checkpoint + checkpointed hybrid sweep) — the incremental-commit instruction worked. Uncommitted `tests/hybrid_resume.rs` snapshotted as `2f21aefb` (UNVERIFIED). No test evidence recorded for any of it.
- C.2 dispatch order rationale recorded in progress (de160fc5 first because 42eac5cc wraps whatever GPU surfaces exist; that ordering paid off — 42eac5cc wrapped both the C.1 hybrid loop AND the de160fc5 topology arm).

## What to do next (AFTER the provider limit resets, 07:30 Helsinki)

- [ ] **42eac5cc rework round 1 (re-dispatch).** Fresh worker into worktree `agent-42eac5cc` (clean at `0a38bc9a`). The two findings VERBATIM are in the progress-file rework_note and in the s8 SendMessage text (recoverable from the transcript of agent aea7b4981feecc4f4); key content: (1) GPU-gated integration test on the PRODUCTION hybrid path with injected OOM at the non-vacuous waterfall (seed `0xDE16_0FC5`, 4 frames, 1/4 errored — the de160fc5 smoke operating point), 3-col assert vs CPU-only reference + `tracing::warn!{batch_id, snr_idx, device_id}` assert, <5s if possible else slow-tier + fast unit test; (2) subprocess exit-status test via a test-only injection flag (checkpoint_compat pattern; KernelErrorInjector needs no GPU); plus the FULL workspace battery (round 1 only ran the gf2-sim tier).
- [ ] **571c11c4 completion worker.** Fresh worker into worktree `agent-571c11c4` (HEAD `2f21aefb`): assess `00367b1a` (drain + checkpoint integration + resume — claimed but UNVERIFIED), finish `tests/hybrid_resume.rs` (3-config 4-col parity, slow-tier) + a fast GPU-gated drain+resume smoke (<5s, gate-visible — the de160fc5 F2 lesson: a [hard] criterion only proven by `#[ignore]`d tests FAILS review unless receipts attest it AND a fast smoke runs in the gate), run the full battery, report column values. Use the de160fc5 completion-worker prompt as the template (it worked well).
- [ ] **Merge serially** (42eac5cc first or whichever is review-clean first); watch conflicts in `executor/mod.rs`, `scheduler.rs` (both branches touch them), `config.rs`. Lead pre-gate review BOTH (Tier 1.5: my two 42eac5cc findings must be closed; 571c11c4 has no formal rounds yet).
- [ ] **Gate each** (pre-warm → settled loadavg → cargo-ci → code-review), close both → **story 9e853c62 (Phase C) closes** (verify story criteria coverage; no story-level gates expected — confirm with `jit issue show 9e853c62`).
- [ ] **Phase D pre-dispatch audit** (the criteria_audit pending item): verify D.1 `8c8302c8` / D.2 `bbf6b6ee` / D.3 `0d9cb8e3` deliverable wording against the AS-LANDED C surface: `Pipeline::run`/`run_with_decoder`/`run_parallel`, `Builder::with_gpu` (wires GpuLdpcBp+DvbT2BchTail into the stage list), `SimulationResults`/`SnrPointResult`, `TopologyExecutor`, `dispatch_with_fallback`, `drain_for_checkpoint`, `diagnostic_dump_dir`. D.2 owns the `--strict-gpu` CLI [hard]; D.3 references B.4's §11 CLAUDE.md block (does NOT re-author) and owns the Pipeline-driven CPU-vs-GPU regression (mode-C `with_gpu(true)` over 6 MODCODs).
- [ ] Then Phase E per handoff-6/7 lists.

## Traps — do not repeat these (NEW this session; s1–s7 traps still apply)

- **The provider session limit kills BOTH a long-running worker AND a freshly-resumed one.** The 42eac5cc rework resume died after 2 tool uses (limit was already exhausted account-wide; resets were 02:30 then 07:30 Helsinki). Before dispatching real work, consider the time-of-day; a dispatch near the limit boundary wastes the dispatch. When a SendMessage resume returns almost instantly with a tiny token count, suspect the limit — check the result text, not just "completed".
- **Worker self-reports of "tests pass" can be scoped narrower than the dispatch battery.** The 42eac5cc Sonnet worker reported "185/185 fast-tier tests pass" — that's the gf2-sim crate only; the dispatch demanded the workspace tier. Read the NUMBER and scope in worker reports (workspace ≈ 4300+, gf2-sim ≈ 200) before treating verification as done.
- **Criterion tests at the FUNCTION level are vacuous for run-level criteria.** "A forced OOM during a hybrid run yields the same fer/frames/errors" cannot be satisfied by a TinyBatch/Identity unit test that produces no columns — the integration test must drive the production hybrid path with injection and compare real column values non-vacuously (B.4-r1 precedent, now recurring; put the expectation IN the dispatch prompt with the operating point spelled out — it was, but verify in review anyway).
- **"Non-zero exit" criteria need a subprocess asserting real exit status** — in-process "the Err propagates, which is how the process would exit non-zero" reasoning has now been rejected twice (5f12e7ff formal finding; 42eac5cc lead finding). The checkpoint_compat subprocess + test-only-bin-flag pattern is the established fix.
- **An `#[ignore]`d-only proof of a [hard] criterion fails formal review** (de160fc5 r1 F2): the green gate must RUN the criterion (fast GPU-gated un-ignored smoke) AND/OR receipts must attest the slow legs. Fast-smoke + receipts is now the standard pair — bake it into every dispatch with GPU-tier criteria (done for 571c11c4's pending completion).
- **Routing contracts must cover ALL members of a class, not the one instance the preset uses** (de160fc5 r1 F1): "GpuOnly → owned stream" meant every GpuOnly type incl. graph-API-only ones; the fix pattern (typed error on unknown members, no silent fallthrough) prevents the contract rotting as new types appear.

## Open questions needing user input

None.

## Reference artefacts

- Epic: `jit issue show f9717e7e`. Design doc §4/§9/§11 current; project plan unchanged.
- de160fc5 landed surfaces: `crates/gf2-sim/src/executor/topology.rs` (TopologyExecutor/DagOutputs/FailurePolicy-pending-merge), stream-aware entry points in `crates/gf2-kernels-hip/src/launch_chacha20_awgn.rs` + `lib.rs` (demap), `gpu-stages-receipts.md` §de160fc5.
- 42eac5cc branch: `worktree-agent-42eac5cc` @ `0a38bc9a` (review-failed tests in `tests/executor_failure_modes.rs`; core in `src/executor/failure.rs`).
- 571c11c4 branch: `worktree-agent-571c11c4` @ `2f21aefb` (snapshot; `00367b1a` is the substantive commit).
- Worker agent IDs (transcripts): 42eac5cc round-1+rework = aea7b4981feecc4f4; 571c11c4 = a5760418db3379038; de160fc5 completion = abd009e218a981e5d, rework = afa45a0c64f973493.
- Unrelated locked worktree `agent-a7c37e2288e3dc230` (issue 82dd7384) — leave alone.

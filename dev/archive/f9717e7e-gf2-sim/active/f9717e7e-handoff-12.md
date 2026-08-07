# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 11

**Date:** 2026-06-12
**Session number:** 11
**Prior handoffs:** `f9717e7e-handoff.md` (s1) … `f9717e7e-handoff-11.md` (s10). Progress: `dev/active/f9717e7e-progress.json`. **All traps from s1–s10 remain in force.**

## Current state

- Epic: `f9717e7e` — `in_progress` (claimed `agent:project-lead`). Main HEAD = this handoff commit (on top of `2f41613d`).
- **Phase D COMPLETE**: D.3 `0d9cb8e3` done (all 3 gates green at `55cebbee`, code-review r3 PASS); story `5d0a3fad` done (all 4 criteria verified). Cross-epic `e4849f07` deps all green.
- **E.1 `acf9b11a` DISPATCHED** (Opus bg worker a5533a7747a02d31a, worktree `agent-acf9b11a` on main `2f41613d`, claimed + in_progress). Running at handoff time.
- Stories: A/B/C/D done; E `a5635da5` backlog (claim when its children start closing or at story close).
- Active claims: epic (`agent:project-lead`), `acf9b11a` (`agent:claude`). No open escalations. Foreign locked worktree `agent-a7c37e2288e3dc230` (issue 82dd7384) — leave alone.

## What just happened (session 11)

- **D.3 `0d9cb8e3` full cycle**: Sonnet worker delivered `tests/dvb_t2_regression.rs` (6 MODCODs, production `Pipeline::run` path, D.1 calibrated points verbatim, 50-frame slow legs + un-ignored smokes, 200-frame off-test receipts `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md` — A=B=C all six, `mean_iters` diff 0.0000 on gfx1030). Lead review: 4 doc-level fixes (off-test command doc, §11 wording, CLAUDE.md bracket-link, project-plan §11 landed-note). Merged `c5a95579`.
- **code-review r1 FAIL** (4-col comparator duplicated vs `tests/common` SSOT) → fixed by delegation through new shared `common::snr_point_to_counters` (moved from `hybrid_resume.rs`). **r2 FAIL** (the new 3-col comparator must itself become the SSOT — 5 hand-rolled sites cited) → exhaustive sweep folded **7 sites across 5 suites** (incl. 2 the reviewer hadn't cited: `pipeline_run_cpu.rs`, `hybrid_scheduler.rs` 4-col) into `common::assert_three_columns_byte_identical_log_mean_iters` (over `WorkerCounters`; `gpu_byte_identity.rs` adapts its local `Counters` via `to_worker_counters`). **r3 PASS** (`86b4abda`).
- **The `tests` gate (bare `cargo test`; D.3 = first carrier since Phase C) exposed two latent cross-test races, both root-caused + fixed in-task**:
  1. `dag_topology.rs` span test: global `Capture` subscriber + assert-while-holding-`MutexGuard` → poisoned mutex cascaded `PoisonError` into 6 sibling tests. Fix `b6d84c07`: poison-tolerant lock sites, snapshot-then-assert, filter captured spans on the test's unique `batch_id=7`.
  2. **tracing-core 0.1.36 `Rebuilder::JustOne` callsite-interest poisoning** (~50%/run loss of `snr_completed` in gf2-coding observability tests). Root cause (verified by instrumented repro + vendored-source read): with ≤1 registered dispatcher, the FIRST-ever hit of an event callsite computes its cached `Interest` from the HITTING thread's current dispatch — a concurrent subscriber-less test caches `Interest::never` for a shared callsite process-wide; events are then skipped before consulting the emitting thread's live thread-local subscriber, until the next `Dispatch::new` rebuild. Fix `55cebbee`: persistent `ANTI_JUSTONE_DISPATCH` (one extra registered no-op dispatcher) in `setup_tracing_guard` forces interest to fold over the registered list (`never.and(always)=sometimes`); plus rayon workers no longer install `Dispatch::none()`; plus `TRACING_SUBSCRIBER_GUARD` serializes the 4 subscriber-creating tests (defense-in-depth). Validated 16/16 clean full-lib bare runs vs ~50% failure pre-fix.
- Closed D.3 (3 gates green at `55cebbee`) → closed story `5d0a3fad` → progress `phase_d_closed_2026_06_12` (commit `2f41613d`). Worktree `agent-0d9cb8e3` removed.
- **Dispatched E.1 `acf9b11a`** after pre-dispatch fact verification at HEAD (API surface confirmed; one correction vs amendment text: `bg{1,2}_base_matrix(z: usize)`; `nr_5g_rate_matched` at `mod.rs:242` flagged to worker for the rate-coverage/non-goal reconciliation with stop-and-report). Dispatch prompt summary in progress.json wave E.1 `dispatch_note`.

## What to do next

- [ ] **E.1 `acf9b11a` worker completes** → lead review (Tiers 1.5–3): scrutinize PROVENANCE.md (URL/version/license — redistribution must be permitted), the external-parser-vs-SSOT direction (parser in tests only), per-(BG, i_LS) coverage, the wrong-i_LS guard's loudness, and isolation under bare `cargo test` (run the gf2-coding lib battery a few times). Merge → gates `cargo-ci`, `tests`, `code-review` (loadavg settled) → close.
- [ ] If the worker STOPS on unobtainable references / license problems / rate-coverage infeasibility → ESCALATE to user (AskUserQuestion), per the issue's escalate-not-relax clause.
- [ ] Then E.2 `e478daa8` (5G NR typestate preset) → E.3 `23d3525f` (GPU tuning ≥200 Mbps [hard] + the a930be7f deferred per-i_LS [hard] criterion: host base+shift→flat-layout expansion + GPU-vs-CPU byte-identity on real 5G NR lifted code) → E.4 `18e69a1a` (aff3ct/IT++ comparison ±0.2 dB [hard]; uninstallable ⇒ escalate-not-relax) → E.5 `110e45cc` (epic close: CLAUDE.md sweep + story `a5635da5` close).
- [ ] At epic completion: Section 10 (coverage map, epic `gate check-all`, completion report from `references/completion-report-template.md`, archive progress file).

## Traps — do not repeat these (NEW this session; s1–s10 still apply)

- **A task carrying the `tests` gate (bare `cargo test`) inherits every latent cross-test race in every binary it newly exercises** — D.3 was the first carrier since Phase C and paid for `dag_topology.rs` (Phase C) and the gf2-coding observability tests (pre-epic). Budget review time for this whenever a gate set includes `tests` after a gap; run the full bare battery EARLY in the cycle, not at gate time.
- **tracing-core 0.1.36 `Rebuilder::JustOne`**: with ≤1 registered dispatcher, a first-ever callsite hit on a subscriber-less thread caches `Interest::never` process-wide for that callsite (events skipped before the thread-local dispatch is consulted). Do NOT remove `ANTI_JUSTONE_DISPATCH` from `setup_tracing_guard` or "simplify" the worker none-dispatch skip — both are load-bearing against this. Repro/validation method: instrumented side-channel file + 16-run battery (`55cebbee` commit message has the numbers).
- **`cargo test` output capture swallows rayon-pool-thread `eprintln`** (capture sink is inherited by threads spawned from a captured test thread; pool threads live forever with a dead test's sink). Debugging concurrency in tests: write to a side-channel FILE (single `write_all` per line — multi-write `writeln!` interleaves), never stderr.
- **Targeted slow-test runs need `--profile slow`**: `cargo nextest run -E 'test(name)' --run-ignored ignored-only` WITHOUT the profile kills at the default profile's 10 s timeout — looks like a test failure but is an invocation error.
- **Introducing a shared helper makes every existing hand-rolled copy a NEXT-round review finding.** When factoring a comparator/adapter into `tests/common`, grep for ALL sites of the same logic (`fer.to_bits()` was the discriminating pattern here) and adopt the helper everywhere in the SAME commit — the reviewer cited 5 sites; the exhaustive sweep found 7.
- **`Dispatch::new` registers a dispatcher; `dispatcher::set_default` of a CLONE does not.** Registration (and the interest rebuild) happens once per subscriber creation — relevant when reasoning about tracing-global state in tests.

## Open questions needing user input

None. (No escalations this session; both review rounds were genuine findings fixed in-loop.)

## Reference artefacts

- Progress: `dev/active/f9717e7e-progress.json` (`phase_d_closed_2026_06_12`; wave E.1 `dispatch_note` = E.1 dispatch facts).
- D.3 landed surfaces: `crates/gf2-sim/tests/dvb_t2_regression.rs`, `tests/common/mod.rs` (`snr_point_to_counters`, `assert_three_columns_byte_identical_log_mean_iters`), `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md`, `.config/nextest.toml` (gpu-serial both profiles).
- Race fixes: `crates/gf2-sim/tests/dag_topology.rs` (`b6d84c07`), `crates/gf2-coding/src/simulation.rs` (`55cebbee`: `ANTI_JUSTONE_DISPATCH`, worker none-skip, `TRACING_SUBSCRIBER_GUARD`).
- E.1 facts: `crates/gf2-coding/src/ldpc/nr_5g/` (`nr_5g` mod.rs:167, `nr_5g_rate_matched` mod.rs:242, `LIFTING_SIZE_SETS`); worker transcript a5533a7747a02d31a.
- Locked worktree `agent-a7c37e2288e3dc230` (foreign, issue 82dd7384) — leave alone.

# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 9 (part 2)

**Date:** 2026-06-11 (late evening; user requested handoff mid-D.2-closure)
**Session number:** 9 (same session as handoff-9; this supersedes its "what to do next")
**Prior handoffs:** `f9717e7e-handoff.md` … `f9717e7e-handoff-9.md`. Progress: `dev/active/f9717e7e-progress.json` (D.2 wave note has the full round-by-round detail). **All traps from s1–s9 remain in force** (handoff-9's list is from earlier this same session).

## Current state

- Epic `f9717e7e` in_progress. Main HEAD = this handoff commit (on top of `5a7be3f4`).
- Phases A+B+C done; C.3 done; D.1 done (all per handoff-9).
- **D.2 `bbf6b6ee` — implementation COMPLETE, two gates to re-run, then close.** Claimed `agent:claude`, in_progress. Worktree `agent-bbf6b6ee` (branch at `5c59408e`) still exists but is now BEHIND main — the final move+delete was lead-direct ON MAIN (`5a7be3f4`); do not dispatch into that worktree without fast-forwarding; simplest is to finish on main (only gates remain).
- Gates on bbf6b6ee: parallelism-pays PASSED (attested 16.79 fps = 10.35×), cargo-ci PASSED at `e575cab3`+gpu-serial commit, code-review FAILED r1 → the blocking structure was fixed at `5a7be3f4` per the user-approved AMENDMENT 2026-06-11b (single binary; legacy gf2-coding binary FILE + its 2 CLI test files deleted; SimulationRunner LIBRARY retained; debt task `9d34359c` rejected obsolete).
- e4849f07 (cross-epic) unblocks when bbf6b6ee closes.

## What to do next (in order)

- [ ] **Finish bbf6b6ee:** (1) full workspace battery at HEAD (`cargo fmt --all -- --check`; clippy all-features AND gf2-sim no-default-features; `cargo nextest run --workspace --all-features --release --profile ci` — expect ~4365 passed with the gpu-serial group; doc tests gf2-sim) — note the deleted legacy CLI tests drop the count by ~15-20; (2) `jit gate pass bbf6b6ee cargo-ci`; (3) `jit gate pass bbf6b6ee code-review` (the three r1 blockers are all structurally resolved by the single-binary amendment — if the reviewer surfaces NEW findings, triage as usual); (4) close bbf6b6ee → story `5d0a3fad` progress; commit state.
- [ ] **Dispatch D.3 `0d9cb8e3`** (Phase D closer; audit done in `phase_d_predispatch_audit_2026_06_11`): regression test {CPU-1, CPU-24, CPU+GPU} × 6 MODCODs (4-col A-vs-B, 3-col A-vs-C §11); carries a `tests` gate (bare cargo test — process-shared-state trap, 48a0db6c lesson); §11 CLAUDE.md block VERIFY-and-reference (B.4 authored it — do NOT re-author); CLAUDE.md campaign-section fuller sweep + dvb_t2_awgn legacy forward-pointer + PLAN.md closing note (note: the binary-path LINE in CLAUDE.md was already minimally fixed at `5a7be3f4`); smoke-knee Es/N0 from `dev/benchmarks/dvb_t2_awgn/smoke/` — BUT recall the D.1 lesson: knee points are decoder/demap-pair-specific, and the topology-path Mutex serialization makes 200-frame stage-driven legs bust 120 s (use the scheduler/`Pipeline::run` path for mode A/B/C — that's what D.3's deliverable names anyway; it does NOT have the mutex problem). Closes story `5d0a3fad` (check story criteria, claim, close).
- [ ] **Then Phase E** per handoff-9's list (E.1 → E.2 → E.3 → E.4/E.5).
- [ ] At epic completion: Section 10 (coverage map, epic gate check-all, completion report, archive progress file).

## Traps — NEW since handoff-9 (s1–s9 traps still apply)

- **Issue bodies can embed physically impossible placement details** (bbf6b6ee named a gf2-coding file as the migration site; gf2-coding→gf2-sim is a dependency cycle). Catch at dispatch: dependency-direction-check any deliverable that names a file in a crate that would need a NEW dependency edge. The two-binary workaround failed formal review (ambiguity + duplication + original-still-legacy); the user chose move+delete.
- **A foreign stash-pop conflict can sit in main's working tree and silently abort merges** (`UU .github/workflows/ci.yml`, "Updated upstream/Stashed changes" markers from the user's ancient stash@{1}). Symptom: `git merge` exits "Exiting because of an unresolved conflict" BEFORE starting; chained `&&` commands after it operate on a stale tree (a perf number was measured against a stale binary this way — discarded). Fix: `git checkout HEAD -- <file>`; NEVER drop the user's stash entries (a failed pop preserves them).
- **Two GPU-gated fast tests scheduled concurrently contend the single gfx1030 and both bust the 5 s cap** they clear in isolation (observed 5.04 s timeouts when the 4365-test battery reached them together). Structural fix landed: nextest `gpu-serial` test-group (max-threads 1) over the GPU-exercising gf2-sim test binaries, ci+slow profiles — scheduling only, no timeout changes. Add NEW GPU-test binaries to that filter list in `.config/nextest.toml`.
- **A "monitoring channel restored" fix can be vacuous twice over**: a thread-local `set_default` subscriber drops all rayon-thread events, AND the gf2-sim lib emits ZERO events during sweeps (grep checkpoint/drain/frame_sim — nothing). Live events require the `frame_observer` seam (`Scheduler::run_sweep_checkpointed`'s third arg) + `set_global_default`. Tests must assert specific event types landed (count > 0 per type), not "file is non-empty JSON" (campaign_start alone satisfies that).
- **Perf re-measurements need an idle DESKTOP, not just no-cargo**: an actively-bursting browser (~20-25% of one core sampled) depressed the campaign fps ~12% with tight variance (looks systematic; diagnostics ruled out code cost — 2 events/run, zero hot-path callsites). Record honestly with conditions; don't chase phantom regressions under browser load.
- **`jit issue reject` takes `--reason`; `jit issue update` does NOT.** And the JIT index lock occasionally times out after heavy gate activity — `jit recover` + retry works.

## Open questions needing user input

None. (Session-9 user decisions, all recorded as issue amendments: 9e853c62 coherent-pair; 8c8302c8 200→50 frames; bbf6b6ee 2026-06-11b single-binary move+delete.)

## Reference artefacts

- D.2 detail: progress.json wave-D.2 note (round-by-round); issue `bbf6b6ee` AMENDMENT 2026-06-11b; receipts `parallelism-receipts.md` (worker entry + lead attestation + final-HEAD confirmation + byte-identity attestation).
- Key D.2 surfaces: `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` (THE campaign binary: 15 flags + `--heartbeat-frames`, live tracing events), `observability.rs` (global subscriber), `tests/campaign_{byte_identity,cli_flags}.rs`, `.config/nextest.toml` (gpu-serial group).
- Worker agent IDs (transcripts): D.2 round-1 = a3b87d3a93e048178; fix rounds = ae50707c25f490504.
- Locked worktree `agent-a7c37e2288e3dc230` (foreign, issue 82dd7384) — leave alone. Worktree `agent-bbf6b6ee` — stale (behind main), remove after bbf6b6ee closes.

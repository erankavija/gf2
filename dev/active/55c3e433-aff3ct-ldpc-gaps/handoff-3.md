# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 3

**Date:** 2026-11-04T04:40+02:00
**Session number:** 3
**Prior handoffs:** [handoff.md](handoff.md), [handoff-2.md](handoff-2.md)

## Current state

- Epic: `55c3e433`, state `backlog` (blocked by its sink; assigned to `agent:jit-execution-lead`).
- Wave in progress: wave 5 (`3ee00fcf`). Its confirmation qualified, and adoption is blocked on an escalation. Wave 6 (`5f401907`) waits on it.
- Children summary: 6 done (`10590c11`, `1d787ed3`, `a72f2b3f`, `4555dfb3`, `0cc6c195`, `2088e0ec`), 1 in_progress (`3ee00fcf`), 1 backlog (`5f401907`), 0 rejected.
- Active claims: `3ee00fcf`, assignee `agent:worker`. No worker is running.
- Open escalations: two, below.
- Progress file: `progress.json` in this directory.
- No timer is armed. The pre-approved window is spent.

## What just happened

- The owner authorized one more rework of `2088e0ec`. Codex `gpt-6.1-sol` replaced the local arm wire types with `tuning_campaign_support::arm::{ArmRequest, ArmResult, PairPosition}`, `CacheState` and `WindowRecord`. The lead removed one test that re-tested the library through a re-export. `cargo-ci` and `code-review` passed, and the issue is done.
- `3ee00fcf` prep: Claude Opus wrote `experiments/aff3ct-ldpc-gaps/families/55c3e433-integrated-adoption.json` and its generated configuration, recorded both builds, laid out the input root and ran the untimed smoke. All six cells passed, and the candidate refused no off-predicate cell.
    - Baseline `gf2-batch-pre-change` is built in the detached tree `target/55c3e433-builds/pre-change` at `87ec416d3`, executable SHA-256 `82f3c72a…dd8`.
    - Candidate `gf2-batch-qc-candidate` executable SHA-256 `9ba08077…4e6`.
- Window 2026-11-04 04:00 to 04:09 Helsinki, job rc 0. The `67194d70` queue line has no worktree and was skipped. Receipts:
    - The pilot is accepted with `qualifies` false (exploratory), receipt SHA-256 `ea8df84f85ca82f2563bf68733634c41aad3a33812ef881ac161ea76eee3f447`.
    - The freeze derived a widest relative half-width of 0.012162 and froze a resolution of 0.02.
    - The confirmation is accepted with `qualifies` **true** and no findings, receipt SHA-256 `87cfeaa2d11de5b8f03d5f5f6a8f83c0975b3dd2e256d25c692854c21c06a18a`.
    - Confirmation cells on the 5900X, at 24 pairs and 99.58% intervals: NR `w1-b1` 9.25 [9.22, 9.29] improved; NR `p12-b8` 8.30 [8.15, 8.33] improved. The other four cells are 0.985 to 1.002, not worse.
- The evidence is preserved in the ignored `campaign-evidence/55c3e433/integrated-adoption/` directory of the agent workspace: `prepared/` holds the job, builds and plan; `run/` holds the receipts, logs and the window job output.
- Adoption dispatch: Codex `gpt-6-astra` stopped at the brief's boundary without changing anything. `dev/tools/tuning-profile-compose` has modes `core-owner`, `algebra-owner`, `coding-owner` and `complete`. All of them copy calibrated sections and verify unchanged wrappers, so none can add the fixed `decode.qc_route_cases` selector. The lead verified this at `dev/tools/tuning-profile-compose/src/main.rs:24`.
- The peer session `just-in-time-6a` reports that jit's tracker format moves to 4 (`2117739f`). See Traps.

## What to do next

- [ ] Get the owner's answers below.
- [ ] On approval of the composer change, dispatch it as a tracked change to `dev/tools/tuning-profile-compose`. Then compose the new production coding profile, which needs a cutover.
    - Keep the calibrated measurement and `source_dirty` false.
    - Remove the superseded `gf2-0a77ecfc-…-815998` production file.
    - Update `arms.json` (`gf2-batch-tuned`), `experiments/aff3ct-ldpc-gaps/src/tuned.rs` and `crates/gf2-coding/tests/support/fresh_tuning_process.rs`.
    - Set the tuned arm's `attach_qc` for NR.
    - Sweep the docs listed in the brief, which is preserved as `prepared/adopt-brief.md` beside the evidence.
    - Decide the copy under `dev/reference_data/tuning-profiles/`.
- [ ] Run `cargo-ci`, `code-review` and `research-review` on `3ee00fcf`. Before review, state in the adoption record which line count `max_added_source_lines` binds: 898 production lines, or 1,882 including tests.
- [ ] `5f401907`: write `55c3e433-adopted-vs-aff3ct` under `comparator-family`, smoke it, and arm a window only with fresh owner approval.
- [ ] Reclaim `agent-2088e0ec`, `agent-1d787ed3`, `agent-a72f2b3f`, and, after the closeout, `agent-3ee00fcf` and the pre-change tree.

## Traps — do not repeat these

The traps in [handoff-2.md](handoff-2.md) and [handoff.md](handoff.md) stay in force.

- **Do NOT hand-assemble the production envelope.** The candidate envelope was hand-assembled. It downgraded the encode measurement to `inherited` and set `source_dirty` true, and `coding-owner` rejects inherited provenance. Production goes through the composer.
- **Do NOT swap the tuned arm's envelope and stop there.** `tuned.rs` sets `attach_qc: false` for `gf2-batch-tuned`, so a new envelope alone never takes QC on the arm's corpus.
- **Do NOT install jit from just-in-time's main.** After `2117739f` it refuses format-3 trackers, including gf2's, until the owner runs the five-repository cutover. Keep `~/.cargo/bin/jit` (built Nov 1).
- **`jit gate check` does not exist.** Use `jit gate evaluate <id> <gate>`. `jit gate status` only reports the last run.
- **Do NOT rebuild in `agent-3ee00fcf` or the pre-change tree before reclaiming.** The build records pin those executables, and the input root lives in the worktree's `target/`.
- **A tracked queue line whose command names an ignored path fails `check-references`.** The `3ee00fcf` line did so from `773ff282e` until `c5b146b96` removed it. Remove a spent line as soon as its window ends, and expect the fault while one is queued.
- **`systemctl --user` needs `XDG_RUNTIME_DIR=/run/user/$(id -u)`** in this harness's shell.
- **The prepared quality files are not in the preserved evidence.** They came from `545a3c814:dev/bench_results/c077a88b/v3-preparation/quality/` and are byte-identical to the files both pilots used. The closing record must say so.

## Open questions needing invoker input

- Question: May `dev/tools/tuning-profile-compose` gain a fixed-selector input, restricted to `decode.qc_route_cases`, so that the confirmed QC route enters production's committed envelope?
  - Context: the confirmation qualified (receipt `87cfeaa2…a18a`). REQ-03 requires a committed envelope that production and the tuned arm install, and no composer mode can add a selector without measured provenance. The composer is shared tuning infrastructure (escalation policy entry 8).
  - Options:
    - A: add the input to `coding-owner`. It preserves every other selector and the measurement provenance, with tests, and production gets a new profile id.
    - B: commit a hand-assembled production profile. This repeats the candidate's provenance defects.
    - C: leave QC unselected, which forfeits a qualified adoption.
  - Recommendation: A. It is the smallest canonical change, `qc-go-rule` fixes the predicate without a measured threshold, and nothing else in the composer changes.
- Question: May a second benchmark window run for `5f401907`'s `comparator-family` pilot and confirmation once adoption lands?
  - Context: the plan requires that campaign when a change is adopted. The pre-approved window is spent.
  - Options: A, approve one window for that family; B, decide after adoption lands.
  - Recommendation: A. The adoption family's job took 8 minutes.

## Reference artefacts

- Epic: `jit issue show 55c3e433`; issues `3ee00fcf`, `5f401907`
- Planning: [plan.md](plan.md) (`adoption-family`, `comparator-family`, `qc-go-rule`)
- Decision records: [qc-route-decision.md](qc-route-decision.md), [check-variant-nomination.md](check-variant-nomination.md)
- Family: `experiments/aff3ct-ldpc-gaps/families/55c3e433-integrated-adoption.json`
- Receipts: pilot `ea8df84f85ca82f2563bf68733634c41aad3a33812ef881ac161ea76eee3f447`, confirmation `87cfeaa2d11de5b8f03d5f5f6a8f83c0975b3dd2e256d25c692854c21c06a18a`

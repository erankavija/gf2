# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 27, wave 4

**Date:** 2026-10-05T09:20Z
**Session number:** 27
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-23.md` in this directory. [handoff-23](handoff-23.md) covers this session through the end of wave 3; read it first.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`).
- Wave in progress: 4 of 7. Every open issue of the wave waits on an invoker decision; no work is dispatchable without one.
- Done in wave 4: `6833c5b5`, `1b034786`, `7d44b71f`.
- In progress and held, each assigned `agent:worker`: `f63a2464`, `63bad95d`, `b2e09d41`, `fa1d8733`, `54e70918`.
- Ready and not started: `4337c02e`, `a87ae828`, `bfc2ceab`, `316150fd`, `dd359005`, `66698c3c`. Backlog: `1362381c`, story `ed3d490e`.
- Open escalations: nine, listed under Open questions and in `progress.json` `escalations`.
- No worker, build, gate or timer is running. Main is clean and its last merged-tree `./scripts/cargo-ci.sh` passes.
- Worktrees kept for the held issues: `agent-9fb40c83` (`f63a2464`), `agent-63bad95d`, `agent-b2e09d41` (clean at its anchor, no commits), `agent-fa1d8733`, `agent-54e70918`. All their branches are merged except `b2e09d41`, which has no commits.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- `6833c5b5`: the dense-parity source-evidence ledger verifies at its recorded commits with unchanged bytes; shared verifier `dev/scripts/verify-source-evidence.py`. Gates passed first round. Review `reviews/6833c5b5-r1.md`.
- `7d44b71f`: every `unsafe fn` and `unsafe` block of `gf2-kernels-simd` carries a safety contract; instruction text unchanged in a content-pinned comparison; all assembly listings regenerated on Rust 1.95. Gates passed. Review `reviews/7d44b71f-r1.md`.
- `1b034786` (filed from that audit): the safe M4RM Gray-table builders reject invalid table sizes and overflowing lengths in every implementation; one contract runs over every builder. code-review F1 closed by rework 1. Review `reviews/1b034786-r1.md`.
- `63bad95d`: untimed first stage merged — route inventory, `gf2_core::dispatch_contract` fallback suite, calibration plan and budget. Nothing frozen, no threshold changed.
- `fa1d8733`: unroll variants removed, launcher guarded, `63bad95d` inventory and both `2037941f` closures regenerated; merged. asm-artefact-present and cargo-ci pass; code-review fails on REQ-03's wording.
- `54e70918`: 29 `gf2-core` equivalence tests run their reference on every host; merged as partial. No gate run.
- `b2e09d41`: worker stopped at the equivalence check; the private checker validates the addendum schema and the shared check does not.
- Filed this wave: `1b034786`, `a87ae828`, `bfc2ceab`, `316150fd`, `dd359005`, `54e70918`, `66698c3c`.
- Two merged-tree CI runs failed on single tests at the 15-second cap while a worker was building (load 10 and 29); both merges passed on a quiet host.

## What to do next

- [ ] Read the invoker's answers to the Open questions and act on each; none of the held issues moves without one.
- [ ] `fa1d8733`: after the REQ-03 ruling, re-run code-review only, then lead review and close.
- [ ] `54e70918`: after the REQ-03 ruling, finish the six `gf2-core` bench tests in `agent-54e70918`, then gates cargo-ci and code-review.
- [ ] `f63a2464`: after the three rulings, rework 2 of 2 in `agent-9fb40c83`, re-run code-review, close or reject; then story `ed3d490e`.
- [ ] `63bad95d`: after the five plan decisions and the window authorization, `4337c02e` pilot and confirmation first, then `63bad95d` freezes and measures; the threshold edit happens in `63bad95d`.
- [ ] Tell the lead of epic fa787f85 or the repository owner that `4c1e441f` and `ad2a6a58` are done, and when `f63a2464` closes (`198aafa3` REQ-01).
- [ ] Reclaim the kept worktrees as their issues close, with `LEAD_CACHE_DIRS=none`.

## Traps — do not repeat these

- **Do NOT write an issue criterion looser than the finding it comes from.** Three issues the lead filed this session failed or stalled on the lead's own wording: `b2e09d41` assumed a pure duplicate from a worker's three-line summary; `54e70918` REQ-03 covered 258 tests when the finding named two files; `fa1d8733` REQ-03 said "ledger" and "input snapshot" where it meant trial ledger and receipt input snapshot. Verify the premise in the code and use the exact terms of sibling issues before filing.
- **Do NOT run merged-tree `./scripts/cargo-ci.sh` while any worker builds.** `acceptance_rejects_every_self_consistent_frozen_fact_replacement` takes about seven seconds alone and hits the 15-second cap at load; the run's failure says nothing about the tree. Guard the whole chain, not only its start: a load check before the run passed and the load reached 29 during it.
- **Do NOT make an edit a worker reports as denied by the permission system, and do not authorize it.** `a87ae828`'s generator pin was denied to the `6833c5b5` worker; it waits for the invoker.
- **Do NOT expect an idle worker to have read a ruling sent while it was reporting.** Three workers repeated a "needs decision" after the decision was in their inbox; a second direct message started them. Check the branch for new commits before assuming progress.
- **Do NOT let a worker rebase while a merge chain is about to take its branch.** `fa1d8733` rebased twice during a chain's first CI run; the chain took the new tip only because the merge had not started. Tell the worker to hold before starting the chain.
- **Do NOT assume the `2037941f` producing closures stay valid.** `cargo-ci`'s `campaign-producing-closure` step checks only the shared manifest; the two family closures enumerate the tree and failed their `--check` three times this session after merges that added source files. Run both `--check` commands after any merge that adds or removes a file under `crates/` or `dev/tools/`.
- **Do NOT regenerate `6e87c436`'s drift or assembly records.** Their generators fail on main after the `1b034786` change; `66698c3c` owns the repair.
- **A `(jit:<id>)` commit that fails its own new test is expected for test-first work** (`1f5157072`, `6d03804a2`); the merged tree passes.
- All traps of [handoff-23](handoff-23.md) and earlier remain in force.

## Open questions needing invoker input

- Question: `f63a2464` — three code-review rulings (confirmation cap, REQ-04 absences, QC design). Stated in full in [handoff-23](handoff-23.md).
  - Recommendation: record no valid QC confirmatory verdict so no family proceeds; amend REQ-04's reading for the recorded absences.
- Question: may wave 4 arm overnight benchmark windows at 02:00 Europe/Helsinki?
  - Context: `4337c02e` and the timed part of `63bad95d`.
  - Recommendation: yes.
- Question: `63bad95d` — the five decisions of `63bad95d/calibration-plan.md` section 8 (transpose lane ownership; matvec threshold mechanism, which changes another epic's producer and validator; carry-save crossover; meaning of tuned-portable; a CI step for the contract without `simd`).
  - Recommendation: the plan's own recommendations.
- Question: `b2e09d41` — the private LDPC plan checker validates the addendum schema; the shared `benchmark-ab-runner check` does not. (A) add schema validation to the shared check as a provenance-freeze exception, then remove the private binary; (B) accept the narrower check, remove, track the gap; (C) reject the bug.
  - Recommendation: A.
- Question: `a87ae828` — allow the edit that pins `make-dense-parity-source-evidence.py` to the ledger's recorded revision, which the permission system denied to a worker, or leave the bug open?
  - Recommendation: allow it.
- Question: `bfc2ceab` — per kernel interface, add release-mode checks to the safe entry point or make the unchecked path an `unsafe` or `_unchecked` item?
  - Recommendation: none before measurement; the issue's REQ-04 asks for the cost of a check on a hot path.
- Question: `fa1d8733` REQ-03 — amend to "No committed receipt, receipt input snapshot or trial ledger changes"?
  - Recommendation: yes; that is what the lead meant and what sibling issues say.
- Question: `54e70918` REQ-03 — (A) narrow to `gf2-core` test targets, fail the six bench tests on a stated precondition, file one issue per remaining crate; (B) keep it and choose one skip mechanism for all 225 remaining tests.
  - Recommendation: A.
- Question: `316150fd`, `dd359005` and the family closures — each adds a step to `scripts/cargo-ci.sh`. Approve those shared-infrastructure changes, and should these issues and `bfc2ceab` stay under this epic or move?
  - Recommendation: approve the steps; re-parent as the owner prefers.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`.
- Reviews of this wave: `reviews/6833c5b5-r1.md`, `reviews/1b034786-r1.md`, `reviews/7d44b71f-r1.md`.
- Calibration: `63bad95d/calibration-plan.md`, `63bad95d/calibration-budget.md`, `63bad95d/route-inventory.md`, `63bad95d/fallback-verification.md`.
- Kernel audit: `7d44b71f/verification.md`, `7d44b71f/survey/unsafe-inventory.json`, `1b034786/verification.md`.
- Held work: `fa1d8733/verification.md`, `54e70918/verification.md`, `54e70918/survey/simd-early-returns.json`.
- `b2e09d41` evidence exists only in the session scratchpad (`b2e09d41-fuzz/fuzz.py`, `plan-comparison.txt`); the worker reproduces it from its clean worktree.

# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 21

**Date:** 2026-09-18T18:30Z
**Session number:** 21
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-15.md` in this directory. Sessions 16 to 20 left no handoff; their record is `progress.json` `notes`.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`; containers stay backlog until their subtree is terminal).
- Wave in progress: 3 of 6 in `progress.json` `waves`, run as a rolling wave of at most four workers.
- Children summary (label `epic:zen3-cpu-performance`): 42 done, 7 in_progress, 17 backlog, 2 rejected.
- Active claims (`agent:worker` unless noted): `18a87159`, `4c1e441f`, `ad2a6a58`, `f63a2464` (queued for the window); `d45aff82`, `e1f9a78f`, `9fb40c83` (`agent:work-9fb40c83`) (merged work, gates owed).
- Open escalations: none. Every ruling of this session is in `progress.json` `escalations`.
- Progress file: `progress.json` in this directory reflects the above.
- `main` carries the reworded history. The pre-rewrite tip `7f995007` is tagged `archive/main-before-subject-rewrite`; the old-to-new map is `subject-rewrite-sha-map.tsv`. Nothing of this epic is pushed.
- Benchmark window `gf2-bench-window-20260919` is armed for 2026-09-19 04:00 Europe/Helsinki. A dry run of the runner's key logic selects nine lines (about 160 minutes): `18a87159` ×4 (`agent-b64dc9c4`), `f63a2464` ×3 (`agent-9fb40c83`), `ad2a6a58` ×1 (`agent-0a357f94`), `4c1e441f` ×1 (`agent-c04dd4ac`). The three `e1f9a78f` dense lines are commented out.
- Worktrees: the four window worktrees above plus `agent-e1f9a78f`, `agent-85fc5ff4` (branch `worktree-agent-d45aff82`), `agent-9fb40c83-r3`, and the foreign `agent-02b8137c-run`. The four window worktrees sit on the old history on purpose.
- No worker and no gate is running. Main is clean; the leak check is clean.

## What just happened

- Recovered session 20 from git, the worktrees and its transcript; it had hung on an unanswered question.
- Invoker rulings: BMI2 form only for the shift candidate; non-timed harness smokes everywhere (REQ-03 of `ad2a6a58` and `4c1e441f` reworded, worker brief amended); window armed for 2026-09-19.
- `c04dd4ac` bracket amended and re-reviewed: plan-review PASS, coverage-preview PASS, breakdown-review PASS on run 3 (after syncing `a0812b83` to the manifest with approval, dropping stale doc links, recording the `d45aff82` external dependency). Created `f8dd4dde` and `00dd43c3`; `a0812b83` re-homed behind `00dd43c3`.
- `85fc5ff4` DONE (`reviews/85fc5ff4-r1.md`). `f8dd4dde` DONE (`reviews/f8dd4dde-r1.md`; rework 1 added the Rust 1.95 record).
- Pre-window harnesses merged with merged-tree cargo-ci green: `18a87159`, `f63a2464`, `ad2a6a58` (smoke converted), `4c1e441f` (six campaign mechanisms shared under `dev/scripts/`).
- `9fb40c83`: code-review round 2 blocked the private smoke driver. Invoker: shared smoke as new task `d45aff82`. Shared smoke merged (`benchmark-ab-runner smoke <plan.json> [--record <path>]`, library `tuning_campaign_support::arm`), c04dd4ac and the logical harness converged, `src/arm.rs` added to the shared `f547c394` closure with CI guard `dev/scripts/check-campaign-producing-closure.py`. `9fb40c83` code-review now PASS; research-review FAIL (three high findings on the dynamic profile); round 3 merged: provenance record from committed material, order-statistic intervals, n = 1 labels. No fresh profile session was needed.
- `d45aff82` code-review round 1 FAIL (logical launcher kept a private staged driver; usage text lacked the contract); rework 1 merged. Invoker then ruled that the shared smoke gains a staged mode; REQ-02 is reworded accordingly. Not implemented yet.
- `e1f9a78f`: six rounds. Rounds 2 to 5 each surfaced one new genuine defect; round 4 added a 112-clause audit against the frozen addendum (`dense-parity-conformance.md`) that found seven more and fixed them. Round 5 found only commit-subject policy. Round 6 moved the dense smoke onto the shared subcommand. Gates not re-run since.
- Commit-subject rewrite (invoker: "fix main's history on a new branch"): 111 subjects over 293 commits reworded, tree byte-identical, 181 citations remapped in lead documents, main swapped, DEC-01 removed from `e1f9a78f`.
- 22 dispatches, all opus except one sonnet; one stall (`w-9fb40c83-r3`, 40 minutes, woken by message).

## What to do next

- [ ] Collect the window. For each of the nine lines read `.agents/bench-window/window.log` and the campaign's own `execution.log` and acceptance summary; never the job rc. Expect `ad2a6a58`'s job to exit non-zero after a sound run: its launcher finalizes the receipt, then rejects a paused-then-complete campaign (`run-axpy-confirmation.sh`, terminal list must equal `[complete]`).
- [ ] Post-window round per measured issue (`18a87159`, `f63a2464`, `ad2a6a58`, `4c1e441f`), in its pinned worktree: commit evidence; for the A/B pair freeze and queue the confirmation before any main merge; then rebase the post-window commits onto the rewritten main with `git rebase --onto <new> <old>` using `subject-rewrite-sha-map.tsv`, verify the tree is unchanged, reword any subject of 72 or more characters, merge with `chore(jit:<id>): merge ...`. Each round also converges its private smoke on `benchmark-ab-runner smoke`, adds `dev/tools/tuning-campaign-support/src/arm.rs` to its producing-input closure, and takes main's shared forms under `dev/scripts/` in conflicts. `f63a2464` also converges `ldpc-plan-check` on `benchmark-ab-runner check`.
- [ ] `d45aff82` rework 2 of 2: staged smoke mode per the reworded REQ-02 (`smoke <plan> --stage <dir>`: session loop, append-only log, checkpoints, cells-per-session pause and resume, zero timing samples, `finalize` refuses the stage; tests bind pause, resume, refusal). Worktree `agent-85fc5ff4`; reset its branch to main first. Then gates cargo-ci, code-review, doc-review.
- [ ] `e1f9a78f`: after the staged mode lands, have the dense launcher use it, re-point the conformance rows that now cite the runner's resume test, then gates cargo-ci, code-review, doc-review. On PASS close it and dispatch `c73ffa25`, which re-keys the three held dense queue lines to itself.
- [ ] `9fb40c83`: re-run research-review (cargo-ci and code-review passed before round 3; round 3 touched generators and one test, so re-run all three). It closes only after `d45aff82` is done (dependency). If research-review rejects the derived provenance record, queue a fresh `v4-r3` profile session with a launcher that records invocation, closure and RNG at run time; if it accepts it, file that launcher change as a small task (see `progress.json` `surfaced_pitfalls`).
- [ ] `00dd43c3` (dispatch after `d45aff82` is done): it starts on the shared smoke and derives the residual arm's label from `residual_shift_route()`; `crates/gf2-core/benches/shifts.rs` hard-codes `bitvec-residual-scalar-*` today.
- [ ] Reclaim `agent-9fb40c83-r3` when `9fb40c83` closes, the other worktrees as their issues close (`LEAD_CACHE_DIRS=none`).

## Traps — do not repeat these

- **Do NOT leave `AskUserQuestion` as the last action of a session without a fallback.** Session 20 hung 12 hours on it. Dispatch everything that does not depend on the answer first.
- **Do NOT merge a worker branch without checking its subjects.** code-review blocked `e1f9a78f` on `merge(...)`, `wip(...)` and subjects of 72 or more characters, and the remedy was a history rewrite. Merges are `chore(jit:<id>): merge ...`; write completion commits by hand (`jit`'s generated `chore: complete issue <id> (<title>)` overflows); list long subjects with `git log --format=%s main..<branch> | awk 'length>=72'` and reword on the private branch; re-subject a lead-preserve commit before its branch merges.
- **Do NOT merge main into a worktree that is pinned for a window, and do NOT merge such a branch into the rewritten main un-rebased.** Its ancestry is the old history; an un-rebased merge drags all 277 old commits back in.
- **Do NOT edit evidence files to remap commit ids.** Source-evidence ledgers, `a203a23c/receipt-reevaluation.json` and launcher logs keep old ids; the tag `archive/main-before-subject-rewrite` keeps them resolvable. Never delete that tag.
- **Do NOT accept a smoke that drives `benchmark-ab-runner run`, and do NOT accept a private smoke driver.** The first is a timed run (9fb40c83 F1); the second duplicates the runner wire even when it dispatches through `validate_arm` (d45aff82 F1). The launcher calls the shared subcommand.
- **Do NOT treat the shared smoke as covering checkpoint/resume yet.** Without the staged mode it writes no stage; `e1f9a78f` REQ-03 names logging and checkpoint/resume.
- **Do NOT expect the window runner to skip a done issue.** It skips only on a `<key>.done` marker; comment a line out to hold it.
- **Do NOT gate right after a merge that renames or deletes linked files.** Run `jit doc check-links --scope issue:<id>` first; breakdown-review and `jit validate` fail on a dangling link. Link files, never directories; a `.rs` with intra-doc links can read as a broken asset.
- **Do NOT add dependency edges to bracket leaves without recording them in the plan's decisions table.** breakdown-review failed on the unrecorded `d45aff82` edges. Use `jit dep add --reduce` when the container's direct edge becomes redundant.
- **Do NOT re-run a reviewer loop on a complex harness finding by finding.** Five `e1f9a78f` rounds each found one new real defect; the clause-by-clause audit against the frozen addendum found seven at once. Order such an audit at the second failed round.
- **Do NOT close a criterion on a worker's statement.** f8dd4dde's Rust 1.95 run existed only in its report; code-review failed REQ-06 until a regenerable record was committed.
- **Do NOT run cargo-ci or a cargo-ci gate at load above about 8.** A calibration test and a GPU test hit the 15-second cap at load 15 to 25. Wait on `/proc/loadavg`; have workers prefix heavy commands with `nice -n 19`; use `CARGO_CI_NO_SCCACHE=1` while workers build (sccache temp-dir race).
- **Do NOT let workers kill by command-line pattern.** One worker took down a sibling's cargo-ci that way. Prompts now allow kills by own process id only.
- **Do NOT trust "waiting on cargo-ci" in an idle notice.** Check the process list at once; `w-9fb40c83-r3` idled 40 minutes on a run that had ended.
- **Do NOT quote a process-kill or process-search pattern in any shell text.** The harness rejects the whole command, even inside a note written to a file.
- **Do NOT chain `for p in "a b"` in zsh expecting word splitting, and do NOT start a foreground `sleep`.** Use parameter expansion or arrays; wait with a background until-loop on a file or on `/proc/loadavg`.
- After adding production sources, regenerate both `2037941f` closures (`make-dense-producing-inputs.py`, `make-logical-producing-inputs.py`); they enumerate the tree.
- A test binary that dispatches arms in-process must not spawn unrelated children in sibling tests: `run_process` makes the process a child subreaper (stated in `src/arm.rs` rustdoc).
- Traps of `handoff-15.md` and its predecessors remain in force (gate staggering, `jit gate status-all` for manual gates, hardlinked `du`, zsh `path`, holistic-review only on containers, no re-run for a lead touch-up).

## Open questions needing invoker input

- Question: keep the four-line addition to `AGENTS.md` that documents the shared-closure guard?
  - Context: `w-d45aff82-closure` added it beside the receipt-snapshot sentence; it states a mechanism, no policy.
  - Options: keep; move to the tools' rustdoc.
  - Recommendation: keep.
- Question: should the frozen per-issue closures of measured campaigns (about 20) ever gain `src/arm.rs`?
  - Context: they are evidence of what a past campaign measured; the CI guard covers the shared closure only.
  - Options: leave as evidence; regenerate.
  - Recommendation: leave.

## Reference artefacts

- Epic: `jit issue show 1a379447`; measurement contract `measurement-contract.md`; worker brief `worker-brief.md`.
- Shared smoke: `dev/tools/tuning-campaign-support/src/arm.rs`, `src/bin/benchmark-ab-runner.rs` (`smoke`, `check`), `tests/protocol_contracts.rs`; issue `d45aff82`.
- Shift story: `dev/active/c04dd4ac-zen3-shifts-and-permutations/` (`plan.md`, `breakdown.json`, `shift-feasibility-record.md`); kernel `crates/gf2-kernels-simd/src/x86/shift_funnel.rs`, dispatch `crates/gf2-core/src/residual_shift.rs`, `dev/active/f8dd4dde/`.
- Dense harness: `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/` (`dense-parity-conformance.md`, `dense-parity-harness.md`, `survey/run-dense-harness.sh`).
- DVB profile: `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-dynamic-profile-provenance.md`, `dvb-interleave-profile-tables.md`.
- A/B families: `dev/active/ad2a6a58/`, `dev/active/4c1e441f/`, shared generators `dev/scripts/campaign_*.py`, `verify-campaign-log.py`, `smoke-campaign-arms.sh`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.
- History rewrite: `subject-rewrite-sha-map.tsv`, tag `archive/main-before-subject-rewrite`.
- Reviews: `reviews/85fc5ff4-r1.md`, `reviews/f8dd4dde-r1.md`.

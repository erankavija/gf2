# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 28

**Date:** 2026-10-05T21:50Z
**Session number:** 28
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-24.md` in this directory. [handoff-24](handoff-24.md) states the nine escalations this session opened with.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`).
- Wave in progress: 4 of 7.
- Done this session: `fa1d8733`, `a87ae828`, `f63a2464`, `3a8ec493`, `66698c3c`, `b2e09d41`, `81afdf8e`, `54e70918`, `316150fd`, and the planning node `2133d15f` of story `ed3d490e`.
- In progress, each assigned `agent:worker`: `63bad95d` (producer sweep merged, no gate run), `4337c02e` (pilot queued), `bfc2ceab` (stage 1b, pilot queued, branch unmerged).
- Ready and not started: `dd359005`, `6605cac2`, `dcfe8386`, and the breakdown node `e28795c1` of `ed3d490e`. Backlog: `1362381c`, story `ed3d490e`.
- Open escalations: `bfc2ceab` (per-interface choice, waits for measured cost). The nine of handoff-24 are resolved in `progress.json` `escalations`.
- Benchmark window: unit `gf2-bench-window-20261006` armed for 02:00 Europe/Helsinki with two jobs, the `4337c02e` and `bfc2ceab` pilots. Nightly windows are authorized.
- Worktrees: `agent-4337c02e` pinned at `2eeccb430`, `agent-bfc2ceab` pinned at `0670ff5ff`, `agent-63bad95d` merged through `e2b80d2e7`. The others in `git worktree list` belong to other epics.
- No worker, build or gate is running. Main is clean; its last merged-tree `./scripts/cargo-ci.sh` passes and `jit validate` passes.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- The invoker ruled on all nine escalations; four issue texts amended as approved (`fa1d8733` REQ-03, `54e70918` REQ-03, `f63a2464` REQ-04, `b2e09d41` Notes), later `316150fd` REQ-02.
- `fa1d8733`: code-review passes on the amended REQ-03. Review `reviews/fa1d8733-r1.md`.
- `f63a2464`: rework 2 records that no family proceeds (QC has no valid confirmatory verdict, corrections C-09); three gates pass. Review `reviews/f63a2464-r2.md`.
- `a87ae828`, `66698c3c`, `3a8ec493`, `81afdf8e`: closed; the last three after one rework each. `scripts/cargo-ci.sh` runs `dev/scripts/check-family-producing-closures.py`, which covers four family closures.
- `b2e09d41`: `benchmark-ab-runner check` validates the addendum against its schema; the private checker is removed. Review `reviews/b2e09d41-r1.md`.
- `54e70918`: no `gf2-core` test returns early on a missing SIMD bundle; `6605cac2` and `dcfe8386` own the kernel and algebra crates.
- `316150fd`: `gf2-core` tests build with default features, `simd` alone and all features; `default-features-core-*` CI steps; `CARGO_CI_STEPS` step filter documented in the script header and `AGENTS.md`.
- `63bad95d`: the five plan decisions applied (transpose lane retained, producer sweep, carry-save excluded, tuned-portable by profile selectors, contract in the default-features CI step); `BitMatrix::matvec_with_route` added; the tuning producer sweeps `bit_matrix.matvec_simd_min_words`; campaign declaration, protocol and manifest committed. All merged.
- `4337c02e`: family `4337c02e-matvec-simd-threshold`, pilot frozen on in-build arms (public `matvec` against `matvec_with_route(Scalar)`), smoked, queued.
- `bfc2ceab`: 24 caller-trusted entry points inventoried; checked wrappers under `--cfg gf2_entry_checked` with the default build unchanged; family `caller-trusted-entry-check-cost` frozen, smoked, queued; `dev/scripts/campaign_plan.py` generalised at its source.
- Story `ed3d490e`: bracket scaffolded (`2133d15f`, `e28795c1`); plan and manifest of 66 tasks merged; plan-review passed on the third run after a full terminal-invariant audit (`terminal-audit.md` beside the plan). Invoker decisions DEC-07 to DEC-12 are in the plan.
- Filed: `6605cac2`, `dcfe8386`, `3a8ec493`, `81afdf8e`.
- Nine worktrees reclaimed.

## What to do next

- [ ] Verify both pilots from their own execution logs, never the job exit code: `.agents/bench-window/window.log`, then each campaign's log (cell-start = cell-complete = the addendum's cell count, every cell `measured`, a terminal `complete`, the acceptance verdict).
- [ ] `4337c02e`: dispatch its stage 2 in `agent-4337c02e`: resolution record, confirmation frozen with the canonical freezer, queue line for the next window; main is merged into the branch only after the confirmation. The issue stays open until `63bad95d` installs or declines the value (REQ-03 to REQ-05).
- [ ] `bfc2ceab`: freeze the confirmation with `run-entry-check.sh freeze`; if the pilot's resolution reaches the 1.05 equivalence margin, bring the numbers to the invoker instead of choosing a margin. After the confirmation, put the per-interface decision table (`bfc2ceab/decision-table.md`) with the measured cost to the invoker, then stage 2. Merge the branch after its timed runs and rerun its generators, both `2037941f` closure generators and its smoke.
- [ ] `ed3d490e`: run the breakdown on `e28795c1` from `dev/active/ed3d490e-zen3-ldpc-frame-simd/breakdown.json` (jit-breakdown bracket path), pass coverage-preview and breakdown-review, then plan waves over the 66 tasks.
- [ ] `63bad95d`: settle the two open questions below, queue the producer campaign from a clean run worktree at the merged commit, freeze the protocol families after `4337c02e`'s outcome, then gates cargo-ci, code-review, research-review.
- [ ] Dispatch `dd359005` (it extends the `default-features-<crate>-*` step pattern; the 1 GB disk guard applies), then `6605cac2`, then `dcfe8386` (its `simd`-off suites need `dd359005`).
- [ ] Tell the lead of epic fa787f85 or the repository owner: `f63a2464` is done, so `198aafa3` REQ-01 is unblocked; its `migration/check.py` also lists the removed `ldpc-plan-check.rs`.
- [ ] File the defect the planner reported: cargo feature `llr-f64` of `gf2-coding` and `gf2-sim` enables nothing.
- [ ] Reclaim `agent-63bad95d` when its issue closes, with `LEAD_CACHE_DIRS=none`.

## Traps — do not repeat these

- **Do NOT answer a plan-review "oversized leaf" finding by splitting only the named leaves.** The gate named four leaves, then four more of the same shape, growing the plan from 38 to 47 with a third round certain. One audit of every leaf against the reviewer's observed standard (one protocol family, one tool, one evidence mechanism per leaf; code apart from permanent documentation; deletion apart from caller migration) passed at 66. Order that audit at the first such failure and give the invoker the expected size before starting.
- **Do NOT have workers run the full `./scripts/cargo-ci.sh` in a worktree.** Three delivery runs took 31 to 47 minutes each (sccache off, job budget split across builders, exclusive test lock, `nice -n 19`); the same contract takes minutes on main. Workers run focused suites; the lead runs the contract on the merged tree with `BUSY_HOST_OVERRIDE=1`, which selects the 60-second `ci-busy` profile `AGENTS.md` names. `CARGO_CI_STEPS='<pcre>'` re-runs single steps and is no verdict.
- **Do NOT state tooling locations as literal `dev/` paths, and do not let a check mode write bytecode.** Four reviews failed on these this session (`3a8ec493`, `66698c3c`, `81afdf8e`, and nearly `316150fd`). Put both rules in every dispatch that touches scripts: locate through `dev/scripts/repository_files.py` or a family `locate.py`; prove `git status --porcelain --ignored` is unchanged from a plain `python3` run.
- **Do NOT expect `jit dep add` between two direct children of the epic.** It is refused as not transitively reduced, and `--reduce` drops the child's direct epic edge. The lead holds such orders; `progress.json` `session_scope.order` lists them.
- **Do NOT gate a plan document after rewriting its links without re-registering it.** `jit doc check-links` reported four missing assets from the first registration; `jit doc remove` then `jit doc add` cleared them.
- **Do NOT merge a branch that deletes a linked file without running `jit validate`.** The `b2e09d41` removal left a link on `3be770d5` that failed repository validation and surfaced as a plan-review advisory.
- **Do NOT merge two branches that both regenerated the `63bad95d` source-evidence ledger and expect a clean merge.** Take one side, regenerate with `63bad95d/survey/make-source-evidence.py` and `make-route-inventory.py`, commit, and regenerate once more: the ledger cites last-change commits, which settle only after the merge commit exists.
- **Do NOT tell a worker to merge main into a worktree whose pilot is queued.** The lead wrote that to `bfc2ceab` and had to withdraw it; the brief's rule (main after the timed runs) holds.
- **Do NOT read the `fixture-leak` step as a tree defect when other suites run on the host.** It counts `/tmp` fixtures before and after; `a87ae828`'s first gate failed on 50 against 51 with every test passing, and the re-run passed.
- **Do NOT judge a diff of a wrapped Markdown list without context lines.** The lead sent `316150fd` back for a sentence that was already on the right bullet.
- **Do NOT rewrite `progress.json` with a different indentation.** One update reformatted the whole file and buried the change in a 4000-line diff; keep `indent=1`.
- **Resume an agent only inside the one-hour agent cache window** (invoker rule); an agent idle longer starts as a new session with a self-contained brief. The agents of this session are past that window by the next one.
- **Messages cross with idle workers.** Four workers asked for a ruling already sent; a second direct message started each. Check the branch for commits before assuming a worker is stuck or has started.
- **A stale seeded cache explains "cannot find crate" after merging main, and it also seeds old binaries**: `agent-bfc2ceab` held a September `benchmark-ab-runner` in `target/release`. A launcher builds the tools it checks.
- All traps of [handoff-24](handoff-24.md) and earlier remain in force, except its "do not make the `a87ae828` edit", which the invoker's ruling resolved.

## Open questions needing invoker input

- Question: `63bad95d` — queue the producer calibration campaign?
  - Context: estimated 205 minutes against a 180-minute session budget; it needs a dedicated clean run worktree and does not depend on `4337c02e`'s value. A paused run resumes only by its run identifier.
  - Options: (A) queue the issue form for one night and the run-identifier form for a second if it pauses; (B) hold until `4337c02e`'s confirmation.
  - Recommendation: A.
- Question: `63bad95d` — the tuning validator has one global field table, so it rejects stages of the earlier campaigns `a83583e0` and `dbd8787d` when they are re-validated with the current script.
  - Options: (A) the validator selects its field table from the stage's own inventory; (B) accept, with the earlier stages validated by the script their receipts pin.
  - Recommendation: A; no CI step exercises it today.
- Question: `bfc2ceab` — check or `_unchecked` per kernel interface.
  - Context: the worker recommends the checked form for all four cost classes, because `gf2-core`, `gf2-algebra` and `gf2-coding` deny `unsafe_code` and cannot hold the block an `unsafe fn` pointer needs. The invoker ruled to measure first.
  - Recommendation: decide after the confirmation, from `bfc2ceab/decision-table.md`.
- Question: should worker worktrees use sccache again?
  - Context: `CARGO_CI_NO_SCCACHE=1` came from a stale-server trap of an earlier session and makes every worktree compile from scratch.
  - Recommendation: no strong preference; unverified whether the trap still applies.
- Question: where does the `llr-f64` defect belong (this epic or outside)?
  - Recommendation: outside; it is unrelated to the epic's criteria.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`; worker brief `worker-brief.md`; measurement contract `measurement-contract.md`.
- Reviews of this session: `reviews/fa1d8733-r1.md`, `reviews/a87ae828-r1.md`, `reviews/f63a2464-r2.md`, `reviews/3a8ec493-r1.md`, `reviews/66698c3c-r1.md`, `reviews/b2e09d41-r1.md`, `reviews/81afdf8e-r1.md`, `reviews/54e70918-r1.md`, `reviews/316150fd-r1.md`.
- Story `ed3d490e`: `dev/active/ed3d490e-zen3-ldpc-frame-simd/plan.md`, `breakdown.json`, `terminal-audit.md`.
- Calibration: `63bad95d/calibration-plan.md`, `63bad95d/matvec-sweep-design.md`, `63bad95d/premeasurement-protocol.md`, `63bad95d/campaign-declaration.json`.
- Matvec threshold: `4337c02e/matvec-lane-threshold-addendum.md`, `4337c02e/cell-table.md` (on branch `worktree-agent-4337c02e`).
- Kernel entry checks: `bfc2ceab/inventory.md`, `decision-table.md`, `measurement-plan.md`, `verification.md` (on branch `worktree-agent-bfc2ceab`).
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.

# Handoff — Field capability traits and profile-driven dispatch (6dc81018) — session 11

**Date:** 2026-09-05
**Session number:** 11
**Prior handoffs:** [handoff.md](handoff.md), [2](handoff-2.md),
[3](handoff-3.md), [4](handoff-4.md), [5](handoff-5.md),
[6](handoff-6.md), [7](handoff-7.md), [8](handoff-8.md),
[9](handoff-9.md), [10](handoff-10.md).

## Current state

- Epic `6dc81018` remains `in_progress`; wave 26 of 28 is open.
- The immediate dependency graph reports 11/12 done. The remaining chain is
  `eaae1b56` (in progress), `a83583e0` (backlog), `dbd8787d` (backlog).
- Assignments remain `agent:jit-execution-lead` on the epic and `agent:codex`
  on `eaae1b56`. The advisory lease was released for this handoff.
- The invoker explicitly requested a handoff and a stop before restarting the
  benchmark. No timing samples or candidate profiles were produced.
- One scope decision remains before measurement: the prepared GEMM observation
  correction extends the exact file boundary in the approved protocol §8.
- [progress.json](progress.json) records the stop and guided retry count 2.
  No retry-counter reset or further scope approval has been received.

## What just happened

- Resumed the preserved worker branch and merged the single-harness work,
  effective M4RM/PLE observations, and shared dedicated-pool convergence.
  Implementation/review used Astra for difficult harness work, Terra for bounded
  Rust work, and Luna for the small pool-helper correction.
- The invoker approved the two-line CI feature correction and retry reset.
  Commit `72d9518e` enables the harness's required features in both dedicated
  CI commands, preserving their execution lock and profiles.
- Built the exact release producer with Rust 1.95. Its first untimed probe
  stopped at child startup. Raw evidence and checksums are committed under
  `dev/benchmarks/tuning_profiles/2026-09-05-eaae1b56-startup-failure.*`.
- Launcher correction `497d4ad4` is merged on main at `3cce5808`. It selects
  the benchmark CLI and libtest adapter explicitly. The actual release
  capability report completed its 12 probes with no timing or output writes.
- Authoritative merged-tree CI result: `jit gate status eaae1b56 cargo-ci`;
  run `e596f29a-0f05-49de-a432-ca3b7937099c`, evaluated at `3cce5808`.
  Code-review and doc-review remain pending for the complete deliverable.
  Review results belong in JIT gates; this handoff does not duplicate them.
- Prepared, but did not merge, GEMM observation commit
  `d4fe0edce990f703e81e096761cb33bb8ce5389d` on `worktree-agent-eaae1b56`.
  Its focused installation/fresh-child checks pass; full merged CI is pending.
- Documentation build with Rust 1.95 succeeded. Incidental public rustdoc-link
  warnings are tracked as `2d65e37f` under the documentation debt container.
- The historical 389 owner, complete envelope, raw log, and manifest retain
  their committed bytes. The current live owner is still the 389 profile.

## What to do next

- [ ] Read the invoker's response to the scope decision below. Do not start
  measurement while that decision remains open.
- [ ] If approved, apply `/tmp/eaae1b56-gemm-observer-scope.patch` from main.
  It combines `git show --format= d4fe0edc` with a §8 boundary addition naming
  `crates/gf2-core/src/field/matrix.rs` and
  `crates/gf2-core/tests/tuning_profile_gemm_install.rs` for effective GEMM
  evidence only. The patch passes `git apply --check` at this handoff.
  If the scratch patch is missing, recover the code from the preserved commit
  and make exactly that boundary amendment. Preserve production dispatch,
  public interfaces, grids, fixtures, and timing parameters.
- [ ] Commit the approved correction and decision; run the configured
  `jit gate evaluate eaae1b56 cargo-ci --json`, then commit its JIT result.
  Refresh document links if their pinned revision needs updating. Reacquire an
  advisory lease with `jit claim acquire` as appropriate; assignment persists.
- [ ] Build from the final clean main revision using protocol §7, or the exact
  extracted script `/tmp/gf2-eaae1b56-build.sh`. Run the resulting executable's
  `--self-check`, `--list-grid`, and untimed `--capability-report` with
  `RUSTUP_TOOLCHAIN=1.95.0 RAYON_NUM_THREADS=4`. Confirm a quiet actual host.
- [ ] Run protocol §7's locked campaign and composer, or
  `sh /tmp/gf2-eaae1b56-measure.sh` after the new build script succeeds.
  The build script writes `/tmp/gf2-eaae1b56-active-stage`; the measurement
  script loads it. The existing pointer names the failed, obsolete stage:
  **do not reuse it**. No commits or repository edits between build and timing.
- [ ] Independently validate the complete new evidence before publication.
  `/tmp/validate-eaae1b56-campaign.py STAGE` is a syntax-checked independent
  validator, not a validated result. It checks logged counts, samples,
  selection arithmetic, forcing, seeds, digests, and envelope identity. It does
  not replace Rust strict-reopen checks or prove historical host exclusivity.
- [ ] Publish the validated receipt/artifacts with every live-reader cutover
  and removal of `PREPUBLICATION_HARNESS_SCHEMA` in one atomic change, following
  protocol §§7–8. Keep all historical 389 artifacts immutable. Run all
  `eaae1b56` gates before completion; only then advance to waves 27 and 28.

## Traps — do not repeat these

- **`cfg(test)` does not distinguish a libtest executable from this benchmark.**
  Cargo's `harness = false` bench also enables it. The committed startup stderr
  records `unknown argument: --exact`; use the explicit entry selection in
  `497d4ad4`. Its handling covers arguments, timing guard, and stdout parsing.
- **Do not benchmark from the previous stage or a dirty/moved revision.**
  `/tmp/gf2-eaae1b56-20260904-211846-793665` contains the failed startup at
  `d85df072`, not publishable measurements. Rebuild after all code, protocol,
  handoff, and JIT commits. Executable path files contain plain text read by
  `cat`, not JSON. Preserve absent-output checks and the exact timeout.
- **Do not call the 12-probe capability report a full-grid validation.** The
  campaign performs the complete declared probes; no full campaign has run.
- **Observe the actual host.** Sandboxed `ps` sees an isolated process tree.
  Use host execution for load checks and measurement. The benchmark and
  composer share the existing exclusive `--full-host` lock; no Cargo under
  that lock and no second private lock domain.
- **Do not publish a schema-token-only cutover.** Temporary v2 acceptance exists
  solely to keep the current 389 live owner readable. Remove it atomically
  with validated v3 artifacts and every current reader, not before measurement.
- **Do not create another review-result document.** The invoker explicitly
  requires JIT gate results as the review authority. Handoffs carry execution
  state, next commands, decisions, and links rather than review reprisals.
- **Prior traps remain in force.** Follow the linked handoffs, including the
  resolved boundary-clipped M4RM decision in handoff 10. Keep the single
  producer, existing pool bridge, and exact deterministic rank-seed fixtures.

## Open questions needing invoker input

- Approve the prepared `d4fe0edc` GEMM observation correction and the two-file
  protocol §8 boundary addition? The patch is
  `/tmp/eaae1b56-gemm-observer-scope.patch`; production dispatch is unchanged.
  Recommend approval, followed by merged CI and a fresh build before timing.
  The execution-lead skill says “Issue scope changes require escalation.”
  The protocol says “The source-path boundary is exact,” so this explicit
  boundary extension is held for the invoker. No benchmark restart is approved
  for this stopped session even if the scope approval arrives.

## Reference artefacts

- Epic and gates: `jit issue show 6dc81018 --json`;
  `jit issue show eaae1b56 --json`; `jit gate status eaae1b56 cargo-ci --all`.
- [Approved protocol](../eaae1b56/premeasurement-protocol.md),
  [selector convention](../7d824b2f/design.md),
  [crate-owned tuning design](../3fa7c9d0/design.md),
  [plan](plan.md), [classification](classification.md).
- Preserved worktree: `.agents/worktrees/agent-eaae1b56`, branch
  `worktree-agent-eaae1b56`, clean at proposed commit `d4fe0edc`.
  Do not discard its unmerged scope patch. Other preserved worktrees are not
  active; do not merge old equivalent patches again.
- [Startup stderr](../../benchmarks/tuning_profiles/2026-09-05-eaae1b56-startup-failure.stderr),
  [raw startup output](../../benchmarks/tuning_profiles/2026-09-05-eaae1b56-startup-failure.log),
  [checksums](../../benchmarks/tuning_profiles/2026-09-05-eaae1b56-startup-failure.sha256).
- Escalation rule:
  [execution-lead skill](../../../.agents/skills/jit-execution-lead/SKILL.md).

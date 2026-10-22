# Handoff — Zen 3 CPU performance (1a379447) — session 43

**Date:** 2026-10-22
**Session number:** 43
**Prior handoffs:** [handoff-37.md](handoff-37.md) and its ordered predecessor chain.

## Current state

- Epic: `1a379447` — backlog; wave 6 of 7.
- Direct dependencies: 32 done, two backlog, one rejected. The remaining
  execution chain is d06a99ed → ed3d490e → e5f8b3ae → 1362381c.
- Active claim: d06a99ed, `agent:worker`, assigned 2026-10-22 19:06 UTC.
- Open escalations: none. The user requests autonomous continuation.
- Progress file: [progress.json](progress.json).
- Timer: `gf2-bench-window-20261023.timer`, 2026-10-23 04:00 Helsinki time.
  It runs the existing benchmark-window runner from the main repository.
- Routing implementation and checkpoint correction are committed. Final CI
  is pending at this handoff checkpoint. Both review gates remain failed on
  the missing current before/after evidence; do not close the issue yet.

## What just happened

- `7988b2ac5`: canonical worker selector and explicit call-time propagation
  across coding, pooling, simulation and live benchmark arms.
- `24db08ef9`: corrected the constructor example found by independent review.
- Focused coding, simulation, portable/native posterior and worker-context
  suites pass; [implementation record](d06a99ed/implementation.md) lists runs.
- `04652b0ee`: checkpoint tests exercise canonical scalar/portable/native
  routes at 1/2/4 workers, including genuine interruption and partial waves.
  Two selected tests pass; independent review confirms the gap is closed.
- Code-review `b96051c6` and research-review `f7415d76` both fail F1: current
  pinned production before/after measurements, including dispatch overhead.
  [Round-one review](reviews/d06a99ed-r1.md) records the findings.
- `cfa4dfecb`: six-cell comparison preparation, two Rust 1.95 native release
  builds and untimed smoke. All twelve arm checks pass with zero timing
  windows and expected routes. Historical receipts and inputs are unchanged.
- The overnight launcher reuses existing pilot, freezer, confirmation and
  acceptance tools. It commits only the generated confirmation addendum,
  plan and derivation between phases. No new protocol or schema.
- Removed one obsolete queued job for completed dbd8787d without a done
  marker. Every other pre-existing queued job has its done marker.
- All workers stopped editing. No timed measurement occurred in this session.

## What to do next

- [ ] Inspect the final CI result recorded after this checkpoint; if absent,
  evaluate d06a99ed `cargo-ci` once with every writer stopped.
- [ ] After the window, inspect `.agents/bench-window/window.log`, the d06 job
  output, and each campaign execution log under
  `dev/bench_results/d06a99ed/worker-routing/`. Do not infer completion from
  the timer alone. A failed or inconclusive result remains evidence.
- [ ] Use [measurement commands](d06a99ed/measurement/commands.md) to project
  the outcome. Commit receipts and all their input snapshots, link artifacts,
  and update the routing record/report with actual intervals and outcomes.
- [ ] Re-evaluate code-review and research-review sequentially after F1 is
  resolved. Perform the cumulative review, then complete d06a99ed.
- [ ] Close the ed3d490e story checkpoint after reconciling its criteria.
  The read-only audit reviewed all 92 pitfalls: measured limits and later
  records resolve applicable historical entries; lane-resume L1 is now fixed.
  Fix the wording in `dev/bench_results/f434ddd3/profile-dry-session.md:4`:
  the diagnostic output has timing fields but is not performance evidence.
- [ ] Claim and execute e5f8b3ae shared-runner consolidation only after the
  story closes. Then execute final integrated scorecard 1362381c, which still
  needs its own integrated holdout and profiling evidence.

## Traps — do not repeat these

- The optimized compatible JIT is
  `target/jit-e2da055a-release/release/jit`; prepend its directory to PATH for
  gates and use `JIT_AGENT_ID=agent:worker`. Installed JIT rejects this repo's
  configuration. Upstream remains 969c4b47; reinstalling is not a solution.
- Compatible JIT captures inputs optimistically. Mutating code, docs or JIT
  state while a gate runs can restart its checker. The initial CI script
  completed, but its result was not recorded before a retry. Root stopped
  the duplicate run. Run the final gate with all writers stopped and no
  concurrent gate/state mutations; do not claim the abandoned run passed.
- Default LDPC selectors do not prove lane test coverage. The original
  checkpoint tests passed through SingleFrame; explicit canonical cases and
  route assertions are necessary.
- Conservative routing satisfies d06 REQ-02, but does not waive the parent
  contract's current before/after requirement. Both gates explicitly reject
  that omission. F1 stays open until measurements exist.
- Keep detached builds in `target/d06a99ed-builds/{before,after}` intact until
  the campaign finishes. Their pinned executables are used directly. The
  recorder requires the historical AFF3CT executable even though these cells
  compare gf2 revisions; prepared target symlinks satisfy that unchanged check.
- The freezer writes addendum before derivation. The launcher repairs an
  interrupted write with `frozen.frozen_utc`, preserving the existing freeze;
  never replace its timestamp on resume. All six cells fit the existing
  first-attempt confirmatory bound. Do not add cells or relax margins after
  reading results.
- Earlier unresolved traps in handoff-37 and its predecessors remain in force.
- The survey registry discovers live catalogue records by the exact basename
  `build-identity.json`. Comparison snapshots must use explicit comparison
  filenames (`before-build.json`, `after-build.json`) and explicit runner
  arguments; they do not replace live catalogue ownership. Registering both
  comparisons as live records caused four CI failures. Renames preserve their
  bytes; no shared checker is changed. The live calibration digest manifest
  also needs canonical regeneration after coding changes; frozen receipt
  snapshots remain untouched.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show 1a379447`.
- [Progress](progress.json), [routing implementation](d06a99ed/implementation.md).
- [Review](reviews/d06a99ed-r1.md), [preparation and resolution table](d06a99ed/measurement/rework.md).
- [Commands](d06a99ed/measurement/commands.md), [window launcher](d06a99ed/measurement/window.sh).
- [Current LDPC report](6230e0b6/findings.md), [measurement contract](measurement-contract.md).

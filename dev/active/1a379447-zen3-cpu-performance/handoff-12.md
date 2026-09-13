# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 15

**Date:** 2026-09-13T20:14Z
**Session number:** 15
**Prior handoff:** [session 14](handoff-11.md)

## Current state

- Epic `1a379447` remains in wave 3 of 7: 19 of 28 wave issues are done,
  three are in progress, and six remain backlog.
- Main is clean at `3d5365c0`.
- `1d4fd63d` is done. Its previously missing cargo-ci gate passed all 27 steps
  with 5988 tests passing, and all five configured gates are green.
- `5cbb6545` is done. Two Sol xhigh rework rounds and three independent Terra
  xhigh reviews converged on the contract-literal negative outcome. The branch
  merged at `8aaac33e`; all five gates pass, cargo-ci reports 5998 passing
  tests, and the latest documentation and research reviews have zero findings.
  The worker worktree is reclaimed and its branch remains as a salvage point.
- `07ca8585` is clean and queue-ready at `0931e66a` in
  `.agents/worktrees/agent-1c602857`. Its two accepted pilots and frozen
  confirmations are restored; all six prepare-only campaign checks pass.
- `53c5a8c0` and `19513245` remain in progress in their preserved worktrees and
  need the queued profile/confirmation data before completion.
- No Astra model was used or required. Recovery and rework used Sol xhigh;
  independent reviews and configured AI gates used Terra high/xhigh.

## Overnight window

- User timer `gf2-bench-window-20260914.timer` is active and waiting for
  2026-09-14 04:00 EEST.
- Its service points to [run-window.sh](bench-window/run-window.sh) with the
  primary checkout as its working directory. The runner exports
  `GF2_BENCH_WINDOW=1`.
- [queue.tsv](bench-window/queue.tsv) contains 17 jobs. Seven current queue keys
  are already complete (71 estimated minutes) and will be skipped. Ten jobs
  remain (155 estimated minutes): seven for `07ca8585`, two for `19513245`, and
  one for `53c5a8c0`.
- Expected finish is about 06:35 EEST if the estimates hold. Each campaign's own
  append-only execution log and receipt, not the orchestration return code, is
  authoritative.

## Immediate next work

1. After the window, collect and validate every pending campaign from its own
   execution log and receipt.
2. Finish `53c5a8c0`: complete profile attribution, findings, generated tables,
   review, merge, gates, evidence links, and closure.
3. Finish `19513245`: decide the vector/matrix outcomes from the confirmations,
   then findings, assembly attribution, review, merge, gates, links, and closure.
4. Finish `07ca8585`: decide all four route families from the restored frozen
   confirmations and profile, perform Tier 1.5 re-review against
   [reviews/07ca8585-r1.md](reviews/07ca8585-r1.md), then merge, gate, link, and
   close. Closure unblocks `f63a2464`.
5. Run the `jit-planning-lead` flows for planning nodes `8d8be934`
   (`2037941f`, mid-range buffers) and `8ed3ac58` (`c04dd4ac`, shifts and
   permutations). Their implementation stories, together with the three open
   measurement issues, block `63bad95d` production calibration.
6. After `63bad95d`, complete the remaining LDPC exploration and final
   scorecard chain ending at `1362381c`, then run the epic's holistic gates.

## Verification and cautions

- `jit validate` passes with 28 pre-existing warnings and no errors.
- The host remains unlocked during active sessions. Do not run timed work before
  the benchmark window or compete with it after 04:00.
- Do not adopt a candidate from a receipt with `qualifies: false`; preserve the
  negative result and retain established production behavior.
- Serialize JIT tracker writes and gate evaluations. Refresh the index before
  committing state if a just-completed JIT command appears clean due to cached
  stat data.
- The 5cbb6545 cache harvest completed despite a second concurrent retry
  reporting rsync code 24 as the first reclamation removed files; the worktree
  is absent and the merged branch is preserved.

## Guidance requested

Recommended: end active work now so the host is idle, let the armed 04:00 window
run, then resume with parallel collection and completion of `53c5a8c0`,
`19513245`, and `07ca8585`. An alternative is to start the two untimed planning
flows before 04:00, with an explicit stop before the window.

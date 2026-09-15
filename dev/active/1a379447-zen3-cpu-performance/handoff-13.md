# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (`1a379447`) — session 16

**Date:** 2026-09-15T04:20Z
**Session number:** 16
**Prior handoffs:** `handoff.md`, `handoff-2.md` through `handoff-12.md`

## Current state

- Epic: `1a379447` — state: in_progress
- Wave in progress: wave 3 of 7
- Children summary: the original wave-3 issues `53c5a8c0` and `19513245`
  are done; `2037941f` and `c04dd4ac` are broken down and active; `07ca8585`
  is also done. The `2037941f` breakdown has 2 done qualification tasks, 2
  in-progress tasks, and 26 backlog tasks.
- Active claims: `3ea122df` by `agent:work-3ea122df` (indefinite,
  `f4cb7e03`); `92385645` by `agent:work-92385645` (indefinite,
  `631ade4f`); `85fc5ff4` by `agent:work_85fc5ff4` (expires
  2026-09-16T03:15Z, `28e72e96`); `9fb40c83` by
  `agent:work-9fb40c83` (indefinite, `0a6eb7b2`).
- Open escalations: `92385645` exceeded two reworks and still permits false
  compiler provenance through `MAKEFLAGS`/`GNUMAKEFLAGS`; `3ea122df` needs an
  invoker-approved issue-description citation repair.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Completed `07ca8585`: packaged the current-source gfx1030 conformance inputs,
  passed every configured gate and independent review r4, closed at
  `220b81db`, and unlocked `f63a2464`.
- Repaired six directory-valued document links on completed `19513245` to
  pinned summary files; repository validation passes.
- Completed `3e83c116`: qualified ISA-L `xor_gen_base` as scalar-only, retained
  dispatched `xor_gen` as unavailable, committed a successful semantic probe,
  passed every gate and independent review, and closed at `e9953c85`.
- Reworked `92385645` twice. The v2 runner pins `/usr/bin/gcc`, clears ordinary
  ambient compiler/linker variables, verifies its retained-cache record, and
  passes all gates. Independent review r2 at `ce3e1957` nevertheless reproduced
  the same false-provenance class through GNU Make's inherited `MAKEFLAGS` and
  `GNUMAKEFLAGS`; the issue remains in progress pending escalation.
- Merged `3ea122df` protocol commit `1c80073f` at `7d04c62f`. Repository,
  documentation, cargo CI, and code-review gates pass. Research-review fails
  only because the issue's existing `cites:IsaL2026` label lacks the literal
  `[IsaL2026]` token in its description.
- Restarted a sandbox-poisoned `sccache` daemon outside the sandbox; the
  unchanged `3ea122df` cargo CI gate then passed.
- Prepared `85fc5ff4` and `9fb40c83` in isolated worktrees and queued their
  6-minute and 15-minute jobs for `gf2-bench-window-20260916.timer`, scheduled
  for 2026-09-16 04:00 EEST.
- Used Sol high/xhigh and Terra high according to task difficulty. No Astra
  model was used.

## What to do next

- [ ] Apply the invoker's ruling on `92385645`. If a focused correction is
  authorized, reset its rework counter, clear `MAKEFLAGS` and `GNUMAKEFLAGS`,
  add a fresh-cache hostile regression, regenerate the semantic record, rerun
  every gate, and obtain a new independent review before closure.
- [ ] Apply the invoker's ruling on `3ea122df`. If approved, append exactly
  `Source references: [IsaL2026].` to its Notes, rerun research and assembly
  gates, run holistic last, obtain independent review, close, and release the
  claim.
- [ ] Commit the current JIT gate/link state together with this handoff and
  progress file; link this handoff to the epic if the local convention requires
  it.
- [ ] At 2026-09-16 04:00 EEST, let the armed window run without competing host
  work. Audit `85fc5ff4` and `9fb40c83` from their append-only logs and receipts,
  then re-engage their workers for findings, review, gates, and closure.
- [ ] Continue only dependency-ready `2037941f` nodes after both qualification
  blockers are closed; preserve topological wave order.

## Traps — do not repeat these

- **Do not treat a generated Makefile's `CC` assignment as proof of the compiler
  make actually invoked.** `MAKEFLAGS` and `GNUMAKEFLAGS` carry command-line
  variable overrides that supersede Makefile assignments; review
  `reviews/92385645-r2.md` and test a genuinely fresh cache.
- **Do not start `sccache` from a sandboxed cargo gate.** The restricted daemon
  survives and makes later escalated cargo runs fail with `Operation not
  permitted`. Stop it and start it outside the sandbox before retrying.
- **Do not add `[IsaL2026]` to `3ea122df` without invoker approval.** The gate's
  requested repair changes issue description text, and execution-lead policy
  item 4 requires explicit approval even for a citation-only edit.
- **Do not run the queued `c04dd4ac` measurements in-session.** The repository
  contract requires all timed work to run under `GF2_BENCH_WINDOW=1`; the timer
  owns the 2026-09-16 slot.
- Prior handoff traps remain in force; read `handoff-12.md` and follow its links
  for older incidents rather than copying them here.

## Open questions needing invoker input

- Question: May the lead perform one focused audited correction on `92385645`,
  resetting the exhausted rework counter?
  - Context: two automatic reworks passed gates, but independent review r2
    reproduced false GCC provenance through inherited make-level overrides.
  - Options: authorize the focused correction; take over manually; reject the
    issue.
  - Recommendation: authorize the focused correction because the bypass and
    required regression are concrete and local.
- Question: May the lead append `Source references: [IsaL2026].` to the Notes of
  `3ea122df`?
  - Context: the existing citation label otherwise causes a deterministic
    research-review failure; this newly created child was not named in the
    earlier batch approval.
  - Options: approve the citation-only edit; provide different issue text;
    reject the issue.
  - Recommendation: approve the exact one-line citation repair.

## Reference artefacts

- Epic: `jit issue show 1a379447`
- Design docs:
  `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md`,
  `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-operation-match.md`,
  `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/isal-comparator.md`
- Planning docs:
  `dev/active/1a379447-zen3-cpu-performance/progress.json`,
  `dev/active/1a379447-zen3-cpu-performance/handoff-12.md`
- Benchmark/result artefacts:
  `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-probe-record.txt`,
  `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv`
- Reviews:
  `dev/active/1a379447-zen3-cpu-performance/reviews/92385645-r2.md`,
  `dev/active/1a379447-zen3-cpu-performance/reviews/07ca8585-r4.md`
- External references: citation registry key `[IsaL2026]`; M4RI qualification
  uses the pinned source identity recorded in `m4ri-operation-match.md`.

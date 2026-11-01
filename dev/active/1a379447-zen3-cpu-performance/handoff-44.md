# Handoff — Zen 3 CPU performance (1a379447) — session 50, second part

**Date:** 2026-11-01
**Session number:** 50
**Prior handoffs:** [handoff-43.md](handoff-43.md) and its ordered predecessor chain.

## Current state

- Epic `1a379447`: wave 7 of 7. 31727c57, 1362381c and 0716ee26 are done. Open: `101604ef`, a dependency of the epic.
- Epic gates: repo-validate, doc-review and research-review pass. holistic-review failed; it has not run since the launcher rework.
- `101604ef` (launcher resume identity): cargo-ci passes. code-review failed three times, so rework is exhausted and it is escalated to the invoker.
- Follow-ups depending on 1362381c, outside the epic: 040bc2b8, c9cb0dd2, be6d6f45, befccab3.
- Worktrees: `agent-1a379447` is at the last rework; `agent-1362381c` is merged. Reclaim neither before the ruling.
- `55c3e433` planning: 228c4691 stays in_progress, blocked on coverage-preview; the fix is in a jit binary that needs the gate key rename first.

## What just happened

- Scorecard merged and gated; see [findings](1362381c/findings.md) and `dev/bench_results/1362381c/scorecard.md`.
- holistic-review demanded that five profile launchers resume across comment edits and that existing logs stay resumable. Three designs each failed code-review: parsed settings; selected commands; whole canonical scripts.
- The last run has two blocking findings. F1: first resume of an older log accepts changed lowercase controls and conditionals. F2: a whole-script identity refuses after an emitted heading changes. Comparing all script text refuses prose edits, and comparing less admits control-flow changes, so no bounded comparison of shell text satisfies both.

## What to do next

- [ ] Apply the invoker's ruling on `101604ef`.
- [ ] Then rerun 101604ef code-review, complete it, and rerun holistic-review.
- [ ] Section 10 completion: report, archive, link, transition. progress `surfaced_pitfalls` were reconciled from recorded dispositions.

## Traps — do not repeat these

- **Do NOT run gf2 `cargo-ci` without `CARGO_CI_NO_SCCACHE=1` until `scripts/cargo-ci.sh:83` is fixed.** A run that starts the sccache daemon leaves it with a deleted `TMPDIR`, which breaks every project on the host. Check `/proc/<pid>/environ` of any daemon afterwards.
- **Do NOT expect a rerun to re-execute a gate on unchanged inputs.** jit reuses the verdict over the same declared inputs, including a failure from contention.
- **Do NOT file a follow-up with `--orphan` and no edge.** Four isolated issues broke whole-repo `jit validate`.
- **Do NOT offer a follow-up that depends on the epic as the answer to a holistic-review finding.** It rejected 101604ef in that form.
- **Do NOT let a worker's background cargo-ci end a turn unwatched.** Four hours were lost this way; message it and set a check.
- A subagent cannot write report files; the lead commits `findings.md` text the worker returns.
- `jit gate status` findings do not resolve from a worktree, because gate runs are ignored by git; paste findings into the brief.
- Earlier unresolved traps in handoff-43 and its predecessors remain in force.

## Open questions needing invoker input

- Ruling on `101604ef`; the `scripts/cargo-ci.sh` fix; the `criterion-1.5x` rename (script copy reviewed; after it, file the stale profile record and `auto = true` items and report the SHA to just-in-time-d0); the worktree seeding fix from jit `b5420aa83`, then file it; docs-mechanical `**/inputs/**`.

## Reference artefacts

- [Findings](1362381c/findings.md), [gap register](1362381c/gaps.json), [progress](progress.json), `dev/scripts/resume_identity.py`

# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 3, third stop

**Date:** 2026-11-04
**Session number:** 3
**Prior handoffs:** [handoff.md](handoff.md), [handoff-2.md](handoff-2.md), [handoff-3.md](handoff-3.md), [handoff-4.md](handoff-4.md)

## Current state

- Epic: `55c3e433`, state `backlog`.
- Wave in progress: wave 5 (`3ee00fcf`). The adoption, routing fix and decision record are on `main`. Gates: `cargo-ci` fails (see the blocker below), and `code-review` and `research-review` fail on one finding.
- Children summary: 6 done, 1 in_progress (`3ee00fcf`), 1 backlog (`5f401907`), 0 rejected.
- Active claims: `3ee00fcf`, assignee `agent:worker`. No worker is running.
- Open escalations: two, below.
- The tracker is format 4 since `321f632ac`, rewritten by the peer session `just-in-time-6a`. The installed `jit` is the format 4 build.
- No timer is armed. One `comparator-family` window is approved, to run after adoption lands.

## What just happened

- The owner approved replacing the old profile path in `d06a99ed`'s description. The lead applied it, relinked `20af7e6e`, and merged the adoption at `69e732775`.
- Codex `gpt-6.1-sol` wrote [adoption-decision.md](adoption-decision.md). It counts 907 production lines, or 1,984 including tests.
- Review round 1: F1 (tuned-arm routing of longer batches, blocking) and F2 (budget exclusion, blocking).
- Claude Sonnet fixed F1 in `0ef28ee1a`: one whole-batch production call, and the route predicted through `batch_route_with_workers`. The fix was merged at `5c347b3c2`, and code-review now reports F1 closed.
- `cargo-ci` fails at its `reference-check` step on `321f632ac`'s `.jit/archive-evidence/container/*.json`: 5,051 faults naming historical `dev/archive` and `dev/active` paths. The lead reported this to `just-in-time-6a`.

## What to do next

- [ ] On the owner's budget ruling:
    - A: add the ruling to [adoption-decision.md](adoption-decision.md), keeping both counts (`@/inv/falsification-preserved`).
    - B: revert production to the pre-change envelope, record the adoption as failed under REQ-04, and close `5f401907` with no campaign.
- [ ] Once `check-references` passes again, rerun `cargo-ci`, `code-review` and `research-review` on `3ee00fcf`, then complete it.
- [ ] On A, `5f401907`: write `55c3e433-adopted-vs-aff3ct` under `comparator-family`. Rebuild and record `gf2-batch-tuned`, because its executable changed. Then smoke it and arm the approved window.

## Traps — do not repeat these

The traps in [handoff-4.md](handoff-4.md) and earlier stay in force.

- **Do NOT edit `scripts/check-references.py` or `.jit/archive-evidence/` to clear the faults.** Both are shared infrastructure that the format 4 cutover owns. Wait for `just-in-time-6a` or the owner.
- **Do NOT read a stale "1 fault" from `check-references` as a regression.** An uncommitted new tracked document faults until it is committed.
- **Do NOT treat the production-only line count as settled.** Both reviewers block on it until the owner rules.

## Open questions needing invoker input

- Question: Does the frozen `max_added_source_lines` 1,500 count test source?
  - Context: the delivery adds 907 production lines, or 1,984 including tests. The contract and the declaration define neither count. Both reviews block.
  - Options: A, production source only, so the adoption stands; B, tests count, so the adoption fails the contract and is reverted, with the result kept as negative.
  - Recommendation: A. The budget derives from the delivery's estimated size as a maintenance cost.
- Question: The format 4 rewrite breaks `check-references` repository-wide. Who fixes it?
  - Context: `just-in-time-6a` reports that the records are correct by design: they record the source and destination of archived documents. It proposes a gf2 fix in `scripts/check-references.py` that treats `.jit/archive-evidence/` as historical tracker data, as the checker already treats `.jit/issues/*.json`, with a self-test case. The checker is shared CI infrastructure outside this epic (escalation policy entry 8).
  - Options: A, file one small gf2 task for that checker change and land it first; B, the owner fixes it directly.
  - Recommendation: A. It is small, and it unblocks every gf2 `cargo-ci`.

## Reference artefacts

- [adoption-decision.md](adoption-decision.md); the receipts are cited there.
- Format 4 commit `321f632ac`.

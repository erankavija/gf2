# Handoff — Zen 3 CPU performance (1a379447) — session 46

**Date:** 2026-10-26
**Session number:** 46
**Prior handoffs:** [handoff-39.md](handoff-39.md) and its ordered predecessor
chain. Session 45 wrote no handoff; its state is recovered below.

## Current state

- Epic: `1a379447` — backlog; wave 7 of 7.
- Wave 7: e5f8b3ae done, eb07aa88 (WP-1) done, ecad1724 (WP-2) in progress: code at `d1748dd39`, gates not re-run since
  the final simplification. New bug 94213540 (slow test) is ready.
  Ready, not dispatched: 31727c57 (WP-3), 1358d6ab (WP-4). Backlog:
  78f0aafd (WP-5), 1362381c (WP-6, the scorecard itself).
- Active claims: ecad1724 (`agent:worker`; the lead finished it after rework 2).
- Open escalations: none.
- Progress file: [progress.json](progress.json).

## What just happened

- Session 45 left e5f8b3ae's Codex output unreviewed and the 1362381c design
  committed (`649974f86`) with six open decisions. Recovered both.
- e5f8b3ae: lead doc fixes on the Codex diff (`cc8f8af69`); canonical build,
  smoke and validation records in the story build tree, made live with
  `--supersedes` ([native](../../../bench_results/e5f8b3ae/native/),
  [portable](../../../bench_results/e5f8b3ae/portable/)); intra replay test
  skips on a rebuilt executable (`b3c903949`). Gates pass, zero findings.
  [Review](reviews/e5f8b3ae-r1.md).
- 1362381c design rulings (invoker): D-1, D-2, D-3, D-6 as recommended; lead
  D-4, D-5. Then the drift record falsified D-3's premise (every receipt
  differs); invoker ruled "criteria minimum": consumed cells by identity with
  drift class, remeasure only FI-1..FI-4. Recorded in the
  [design](1362381c/design.md) Decisions and in 1358d6ab REQ-03 (amended).
- Created WP-1..WP-5 (eb07aa88, ecad1724, 31727c57, 1358d6ab, 78f0aafd).
  Sibling edges are refused under transitive reduction; the order is held in
  `session_scope.order` of the progress file.
- eb07aa88 (Codex gpt-6.1-sol): [workload set](1362381c/workload-set.json).
  Gates pass (advisory finding: `jit item show` rejects `[docs-mechanical]`).
- ecad1724 (Claude opus): two reworks; gate F1 held three times on 35
  LDPC-arm receipts with no pinned production content. Invoker amended
  REQ-01/REQ-02 with class `unresolvable` (2026-10-26); the lead implemented it
  and then fixed later review rounds directly (kernel sources, test targets,
  added files package-wide, build-script native sources). Review then demanded
  finer test-code separation (lexing, `#[cfg(test)]` items); the invoker ruled
  "no over-engineering": a package's measured content is all its tracked
  compiled sources with inline test code, minus `tests/`, `benches/`,
  `examples/`. Recorded in the issue Notes; test-stripping code removed;
  deleted pinned sources classify `removed`. Every resolvable receipt is
  `code-differs`, 35 are `unresolvable`.
  [Drift record](1362381c/production-drift.json).
- Filed bug 94213540: `acceptance_rejects_every_self_consistent_frozen_fact_replacement`
  takes 8.3 s alone and hit the 15 s kill in 2 of 6 cargo-ci runs.

## What to do next

- [ ] ecad1724: run cargo-ci, code-review, research-review sequentially on
  main. A finding that asks for finer test-code separation contradicts the
  recorded invoker decision in the issue Notes: escalate, do not implement.
  Other findings: fix. Then lead review record and `done`.
- [ ] Bug 94213540 (Codex gpt-6-luna xhigh fits: narrow, exact criteria).
- [ ] Dispatch WP-3 (31727c57) and WP-4 (1358d6ab) in parallel from the drafted
  briefs [31727c57](1362381c/dispatch/31727c57-brief.md) and
  [1358d6ab](1362381c/dispatch/1358d6ab-brief.md); re-read each issue first and
  prefix the worktree header from `dispatch-worker-worktree.sh`. Vendors:
  WP-3 Codex gpt-6.1-sol high, WP-4 Claude opus (append the return contract).
- [ ] Their in-session work ends with validated launchers and queue lines; the
  lead appends queue lines to [queue.tsv](bench-window/queue.tsv) and arms a
  window only with invoker approval.
- [ ] WP-5 after WP-3/WP-4 confirmations; WP-6 (1362381c) last.

## Traps — do not repeat these

- **Do NOT chase reviewer precision past the criteria's decision value.**
  ecad1724 ran seven gate rounds on test-code boundary edge cases that changed
  no classification. Ask the invoker once the same defect class recurs.

- **Do NOT compare executable digests against the arms catalogue as a
  proxy for production-code drift.** Code and research review rejected it
  (ecad1724 gate F1, round 3). Receipts that pin no production content are
  `unresolvable`.
- **Do NOT write an arms build record without `--supersedes`.** The arms
  tooling treats every committed `build-identity.json` as live; a second
  record of the same arms fails `test_make_family.py`,
  `test_record_arm_smoke.py`, `test_record_peak_memory.py`. Each live record
  also needs `smoke.json` (`record-arm-smoke.py`) and `validation.jsonl`.
- **Do NOT build the native arms outside `.agents/worktrees/agent-ed3d490e-run`.**
  The recorder refuses: "no target file carries the pinned executable of the
  catalogued arm aff3ct-flooding-nms-f32". That tree is now on branch
  `worktree-agent-e5f8b3ae-run`.
- **Do NOT run a script with `--help` to read its usage.**
  `dispatch-worker-worktree.sh --help` created a worktree named `agent---help`.
  Read the header instead.
- **cargo-ci timeouts under load:** one run failed on a 15 s kill in
  `tuning-campaign-support::protocol_contracts` plus its leaked fixture; a
  re-run passed. Re-run before treating it as a regression; never widen the
  budget.
- **The JIT binary is `target/jit-e2da055a-release/release/jit`** in the gf2
  checkout (handoff-39's path is relative to gf2, not just-in-time). The
  installed `jit` 1.0.0 rejects `[docs-mechanical]` and the gate registry.
- Earlier unresolved traps in handoff-39 and its predecessors remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- [Design](1362381c/design.md), [workload set](1362381c/workload-set.json),
  [drift record](1362381c/production-drift.json),
  [e5f8b3ae review](reviews/e5f8b3ae-r1.md).

# Handoff — Zen 3 CPU performance (1a379447) — session 48

**Date:** 2026-10-27
**Session number:** 48
**Prior handoffs:** [handoff-41.md](handoff-41.md) and its ordered predecessor chain.

## Current state

- Epic: `1a379447`, backlog; wave 7 of 7; 36 of 41 children done.
- Wave 7: 31727c57 (WP-3), 1358d6ab (WP-4) and 78f0aafd (WP-5) are prepared,
  and all their timed work is queued. 1362381c (WP-6) is waiting on all three.
- Window: `gf2-bench-window-20261028.timer` (user systemd), 2026-10-28
  04:00 Europe/Helsinki, runs [run-window.sh](bench-window/run-window.sh) over
  [queue.tsv](bench-window/queue.tsv): FI-1..FI-4, then the 78f0aafd decoder,
  BCH and the three restored historical series, about 209 estimated minutes.
- Active claims: 31727c57, 1358d6ab, 78f0aafd (`agent:worker`). No worker runs.
- Pinned worktrees (do not reclaim, do not merge main into them):
  `agent-31727c57`, `agent-1358d6ab`, `agent-78f0aafd` (holds the restored
  bytes under `.agents/ext/78f0aafd/restored`), `agent-ed3d490e-run`.
- Open escalations: none.
- Progress file: [progress.json](progress.json), `session_scope.session_48`.

## What just happened

- Invoker rulings (progress `session_48.invoker_rulings`). Q1: arm the
  window tonight; armed. Q2: (a) rebuild each historical executable and profile
  it on digest match, with (b) integrated-code fallback under an amended REQ-02.
  Q3: 53f5b940 and ff10cb51 stay outside the epic.
- 78f0aafd Q2 (Claude opus; `3a67d955c`, `587c80bd9` on `agent-78f0aafd`):
  all five historical executables were rebuilt with matching digests
  (`78f0aafd/restoration.json` on `agent-78f0aafd`), so there was no fallback and
  no REQ-02 amendment. Lead ruling: the restored arms predate the zero-window
  smoke position and refuse it; accepted, because their receipts establish the
  wire contract. The three series replace the held lines in the queue
  (`a7666ee6b`). Links are held in
  [pending-links/78f0aafd-q2-links.tsv](pending-links/78f0aafd-q2-links.tsv)
  until merge.
- Invoker-directed infrastructure fix (`5ab2964b5`): the docs-mechanical settings
  moved to `contrib/gates/docs-mechanical.toml`, and the redundant gate `auto`
  field was dropped. The installed `jit` 1.0.0 now reads `.jit/` (`doc dir`,
  `gate list`, `item show`, `status`). docs-mechanical passes;
  `622521f6e` fixed three broken links in epic records.
- `jit recover` and `jit validate` exceed jit's 512 MiB capture cap, because
  they load every referenced document. This is jit bug 55e3d3fd in the
  just-in-time repo; the jit session will report the fix commit and its
  install step (`./scripts/install-jit.sh`).

## What to do next

- [ ] After the window, read `.agents/bench-window/window.log`, each job's
  `.out` and each family's execution log, verifying from the logs rather than
  from exit codes. Commit the receipts with their input snapshots on each pinned
  branch, then replay them onto main (handoff-17 recipe). Regenerate the
  schema-version index and the family closures. Link the outcomes, plus
  `pending-links/78f0aafd-q2-links.tsv`, after the merge.
- [ ] If a restored historical series failed in the window, read its `.out`
  first: the zero-window smoke never exercised these arms.
- [ ] Gate 31727c57, 1358d6ab and 78f0aafd in this order: cargo-ci,
  code-review, research-review. Then regenerate `78f0aafd/accounting.*` and the
  workload set after the FI receipts land.
- [ ] Then WP-6 (1362381c), the scorecard. Next dispatch is a Codex worker
  (alternation; the last worker was Claude).

## Traps — do not repeat these

- **Do NOT take the 78f0aafd worker's branch cargo-ci failure
  `survey-test_record_confirmatory_bound.py KeyError: 'ledger'` as a defect on
  main.** The branch base `223295d5d` still carries
  `31727c57/{core,decoder}/confirmatory-bound.json`, which main renamed. The
  test passes on main, and the merge takes main's rename.
- **Do NOT file a provenance gap for `rustflags: null` on a receipt arm with
  `build: native`.** `BuildIdentity::Native`
  (`dev/tools/tuning-campaign-support/src/protocol.rs`) records host
  targeting, and `rustflags` is optional. The worker's report and
  `78f0aafd/report.md` call it a conflict; research-review may raise that
  wording at gating.
- **Do NOT rewrite progress.json with `indent=2`.** The file uses one-space
  indentation, and a reformat touched about 7k lines. Use
  `json.dumps(p, indent=1) + '\n'`.
- **Do NOT read a pipeline's `$?` as jit's exit status in zsh.** It is the last
  command's status; use `${pipestatus[1]}`.
- **Do NOT delete document references to fit under jit's capture cap.** Run
  `recover` and `validate` with `target/jit-e2da055a-release/release/jit`
  until 55e3d3fd lands. For every other command, the installed `jit` now works,
  which resolves handoff-40's binary trap.
- **Worker background cargo-ci leaves the worker idle without its report.**
  Message the worker to collect the result.
- The `cd` trap from handoff-41 recurred twice this session. Earlier unresolved
  traps in handoff-41 and its predecessors remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show 1a379447`
- [Design](1362381c/design.md), [drift record](1362381c/production-drift.json)
- [78f0aafd report](78f0aafd/report.md) (restoration sections and `restoration.json`
  on `agent-78f0aafd` until merge)
- [31727c57 report](31727c57/preparation-report.md), [queue](bench-window/queue.tsv)

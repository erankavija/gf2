# Handoff — Zen 3 CPU performance (1a379447) — session 47

**Date:** 2026-10-27
**Session number:** 47
**Prior handoffs:** [handoff-40.md](handoff-40.md) and its ordered predecessor chain.

## Current state

- Epic: `1a379447` — backlog; wave 7 of 7.
- Wave 7: eb07aa88, ecad1724 and 94213540 are done. 31727c57 (WP-3),
  1358d6ab (WP-4) and 78f0aafd (WP-5) have their in-session preparation merged
  on main, and their timed work is queued. 1362381c (WP-6) is backlog behind
  all five.
- Every remaining epic task waits for the overnight benchmark window, and
  arming the window needs the invoker's approval (Q1 below).
- Active claims: 31727c57, 1358d6ab and 78f0aafd (`agent:worker`). No worker
  is running.
- Pinned worktrees (do not reclaim, do not merge main into them):
  `agent-31727c57`, `agent-1358d6ab` (the BCH arms are built there and their
  path enters recorded digests), `agent-78f0aafd`, and `agent-ed3d490e-run`
  (the FI-2 and FI-3 plans run the arm executables built there).
- Progress file: [progress.json](progress.json).
- Follow-ups outside the epic: 53f5b940 (shared kill-safe scratch;
  convergence condition: after the FI window results land) and ff10cb51
  (profile launcher resumption test, a pre-existing failure, plus its CI
  step).

## What just happened

- ecad1724: re-gated. Code-review raised two more defect rounds in include-target
  and pinned-role selection, and the lead fixed both. The fixes were deleted
  test targets, test-only pins, unavailable bytes masking known differences,
  include targets outside packages, and string continuations. All gates pass
  with zero findings. [Review](reviews/ecad1724-r1.md). The design's D-3
  premise now names the `unresolvable` rows.
- 94213540: Codex luna split the test into fifteen. The lead reverted the
  worker's fixture-prefix rename, which hid trees from `fixture-leak`. Claude
  sonnet added the `Reaped` kill-safe guard. The lead fixed the
  probe-orphaning finding and cited 53f5b940 as the tracked
  convention-convergence exception. Done. [Review](reviews/94213540-r1.md).
- 31727c57 (Codex sol, two dispatches): FI-1 core and FI-2 decoder families
  are prepared and smoke-validated. The lead removed `nice -n 19` from both
  timed launchers and renamed the raw `--next-attempt` records
  `next-attempt.json`, because the family survey test reserves
  `confirmatory-bound.json` for the canonical recorder.
- 1358d6ab (Claude opus): FI-3 AFF3CT LDPC and FI-4 BCH families are prepared.
  The external BCH encoders are wrapped as runner arms whose codewords equal
  gf2's bit for bit. `Bchlib2026` is in `.jit/references.toml`. Ledgers are
  created at the first timed run (lead ruling).
- 78f0aafd (Codex sol): the decoder case file, the `ldpc-batch-profile`
  build record, an untimed smoke, allocation witnesses, the accounting
  generator and the series launchers are in place. The decoder and BCH
  series are queued. Three series are held behind Q2.
- Lead rulings (progress `session_scope.rulings_session_47`): the brief's
  "no ledger/addendum change" means existing records only. P-20 counts
  confirmatory plus holdout cells: pilots may cover every named configuration,
  and the freeze selects at most six.
- cargo-ci passes on merged main at `d9f22e29f` (recorded for 78f0aafd, 31727c57, 1358d6ab).
- [Queue](bench-window/queue.tsv) holds FI-1..FI-4 and two profile series:
  about 160 estimated minutes.

## What to do next

- [ ] With invoker approval (Q1), arm a window over the queue. Start it only
  when `/proc/loadavg` is below 2 and no `rustc` or `cargo-ci.sh` is in
  `ps -eo comm`.
- [ ] After the window, read `window.log`, the job outputs and each family's
  execution log. Commit receipts with their input snapshots on each pinned
  branch, then replay them onto main (handoff-17 recipe). Regenerate the
  schema-version index and the family closures, and run `jit doc add` on the
  outcomes.
- [ ] Gate 31727c57, 1358d6ab and 78f0aafd (cargo-ci, code-review,
  research-review, in that order). Then regenerate `78f0aafd/accounting.*`
  and the workload set after the FI receipts land.
- [ ] Apply the Q2 ruling to the three held profile series.
- [ ] Then WP-6 (1362381c), the scorecard.

## Traps — do not repeat these

- **Do NOT write "no ledger, addendum or frozen record changes" in a
  preparation brief.** Both workers read it as forbidding the new pilot
  addenda and ledgers they had to create; one stopped blocked. Write
  "no change to an existing ...".
- **Do NOT put "prefix heavy commands with `nice -n 19`" in a brief without
  scoping it to builds.** A worker put it in front of the timed campaign launcher.
- **Do NOT accept a fixture-leak fix that renames the fixture prefix.** The
  Codex worker moved the trees outside the `fixture-leak` inventory pattern and
  called it cleanup; that is a bypass of the check.
- **Do NOT store raw `benchmark-acceptance --next-attempt` output as
  `confirmatory-bound.json` beside a family ledger.**
  `test_record_confirmatory_bound.py` reads that name as the canonical
  recorder's output and fails cargo-ci.
- **When a library `src/` change is blocked by pinned arm digests, file the
  convergence issue before the first code-review.** A test-local helper that
  duplicates `tuning_campaign_support::scratch` fails convention-convergence
  unless it cites a tracked issue (94213540 round 2).
- **Do NOT `cd` into a worktree or survey directory in the lead shell.** It
  re-anchored the session twice this session; the `cd` back to the primary
  checkout worked.
- **Expect `jit` writes to exceed 120 s under load** (claims, `doc add`, `dep add`
  while another project's CI ran). Run batches in the background and check
  `git status` before committing.
- Earlier unresolved traps in handoff-40 and its predecessors remain in force.

## Open questions needing invoker input

- **Q1. Benchmark window.** Arm a window (systemd timer, as on 2026-10-25)
  for the queued FI-1..FI-4 campaigns and the decoder and BCH profile series,
  about 160 estimated minutes? Recommended: yes, the next night.
- **Q2. Historical-code profiles (78f0aafd REQ-02).** The NR-encoder,
  byte-field and polynomial gap cells measured executables whose exact bytes
  no longer exist at their recorded paths. A current build differs. Options:
  (a) rebuild each measured executable from its receipt's producing snapshot
  and profile it only when its digest matches the receipt, leaving the rest
  unresolved. (b) Profile the current integrated code and label it as
  integrated, consistent with consuming cells by identity with their drift
  class; this needs a REQ-02 wording amendment. (c) Cite related profiles as
  context only, which leaves REQ-02 unmet for those cells. Recommended:
  (a), with (b) for any cell whose digest cannot be reproduced.
- **Q3.** Bring follow-ups 53f5b940 and ff10cb51 into the epic, or leave them
  outside it? Recommended: leave them outside.

## Reference artefacts

- [Design](1362381c/design.md), [drift record](1362381c/production-drift.json),
  [queue](bench-window/queue.tsv),
  [31727c57 report](31727c57/preparation-report.md),
  [78f0aafd report](78f0aafd/report.md).

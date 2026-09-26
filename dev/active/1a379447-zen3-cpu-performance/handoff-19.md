# Handoff — Zen 3 CPU performance (1a379447) — session 24

**Date:** 2026-09-26
**Session number:** 24
**Prior handoffs:** Ordered list in [progress.json](progress.json), through [handoff-18.md](handoff-18.md).

## Current state

- Epic `1a379447`: tracker state `backlog`, assigned to `agent:jit-execution-lead`; execution wave 3 of 7 remains in progress. Scope is current wave only.
- Direct wave entries: 12 done, 2 rejected, 2 backlog stories (`2037941f`, `c04dd4ac`). Their 11 remaining descendants are recorded in `session_scope`; 5 are in progress and 6 backlog. No worker remains active.
- Lead leases: dense harness lease released after completion; DVB lease `84a600ca-3232-4126-97d3-86ff070c64ce` renewed through 14:39 UTC; dense baseline lease `b2c7b3a8-1ef6-4da0-bd18-dcf453031536` expires about 14:07 UTC. Query live claims before mutations.
- The one-shot user timer `gf2-bench-window-20260927.timer` is active for **September 27, 02:00 Europe/Helsinki** (September 26, 23:00 UTC). Subsequent overnight windows are authorized as needed for this wave, but no later timer is armed.
- Eleven pending enabled jobs total an estimated 208 minutes. DVB's fresh profile is held pending the round-6 decision below. Its extra 12 minutes would bring the estimate to 220 minutes. M4RI and later independent confirmations remain disabled.
- Progress: [progress.json](progress.json). Wave 3 is not complete.

## What just happened

- Interview resolved dense round 7, DVB round 4 and overnight scheduling; user then changed the start to 02:00. Read-only wave audit found no further architecture decisions. Sol high/xhigh workers only; no Astra permission or use.
- Closed `e1f9a78f`: original launcher argv is captured before parsing and Bash-quoted in the actual log. The fixture tests the actual parser/log block; empty, punctuation/space and newline cases fail on the base and pass after repair. Fifteen historical subject findings remain outside main ancestry. Merge `2e2e28f99`; CI, code and doc gates pass; closure `8a9d49c79`. [Lead review](reviews/e1f9a78f-r7.md).
- Merged DVB round-4 preparation `9d78c7976`: nine repetitions collect counters and sampled hot data; interval/provenance generators support the fresh run. Release validation and shared zero-timing smoke passed; existing paired evidence retained. Code review reported the annotation option as unsupported.
- User approved focused DVB round 5. Removed the flagged option from both annotation entry points at worker `71df9c69a`, merged `b7b775c64`; CI passes. Actual retained-data checks show both command forms succeed and the parser accepts the simplified output (148 rows, 16 with samples). The contradiction to the help-only gate diagnosis is preserved in [round-4 review](reviews/9fb40c83-r4.md).
- DVB round-5 code gate `ec204e2a-79fb-4494-b78d-bd71f838038d` fails only on the generated provenance's stale current-renderer hash. The existing generator produces a one-line hash update. Round 6 was presented to the user and remains pending. [Lead review](reviews/9fb40c83-r5.md).
- Prepared `c73ffa25` with Sol xhigh. Worker `815010276` is clean and pinned in `agent-c73ffa25`; merge `efaea4eaf`, CI passes. Release build, 50 harness tests, boundary oracles, shared staged smoke, closure checks, profile prepare, shell checks and read-only perf help/list checks pass. Both baseline input closures contain 233 committed clean inputs. [Preparation](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-baseline-preparation.md).
- Dense code gate `76ce431c-bc47-4942-b897-0e0c7f9c5bad` identifies the three expected incomplete deliverables: baseline receipts, measured profile, and profile-supported portfolio/no-candidate outcome. They remain open; the scheduled jobs supply their prerequisite data. [Lead review](reviews/c73ffa25-preparation.md).
- Dense queue: isolated baseline 20 minutes, allocated baseline 60 minutes, then nine-pass six-cell profile 12 minutes. Only the two gf2 baseline families moved to c73; the M4RI comparison belongs to `50f0bd42`.
- No measurements, perf samples, host locks or benchmark-window environment overrides ran in this working session. Offline annotation read existing samples only.

## What to do next

- [ ] Check the user's response to the round-6 question. If approved, run `survey/make-profile-provenance.py` on the retained v4-r2 dynamic profile to regenerate its sibling provenance Markdown. Verify only the current renderer hash changes. Commit under `jit:9fb40c83`, rerun code review, then restore the fresh DVB queue row when no implementation finding remains. Do not rerun research review before the fresh evidence exists.
- [ ] Preserve all pinned queued trees. Dense baseline is `agent-c73ffa25` at `815010276`; DVB is `agent-9fb40c83-r4` at `71df9c69a`. Prior prepared trees remain `agent-00dd43c3` at `3c08c3494`, `agent-65c0e13d` at `8f2875d8c`, and `agent-bc091474` at `2d33ac5ca`.
- [ ] After the 02:00 window, inspect the canonical orchestration log and each campaign's authoritative execution log, terminal status, checkpoints, raw samples and acceptance result. Preserve negative and unavailable outcomes. The timer alone does not prove execution.
- [ ] Commit complete receipt inputs, including ignored lockfiles where required, and refresh generated receipt indexes. Merge sequentially with merged-tree CI after every merge before another main commit. Follow old-history replay constraints from prior handoffs.
- [ ] Complete c73's baseline/profile interpretation and bounded portfolio or no-candidate disposition. Do not choose a new fusion from assembly alone. Rerun its gates after fulfilling the three findings.
- [ ] Continue the wave DAG: 65c + bc → fcb; bc → a1ad; c73 + fcb → 50f → 6e → 2ad (also a1ad) → 2037941f. 00dd + 9fb → a081 → c04dd4ac. Finish both stories before advancing from wave 3.
- [ ] Freeze confirmations only after committed pilot evidence supports them. Schedule subsequent 02:00 Helsinki windows as needed within this wave's authorization. Keep f63a2464, ad2a6a58 and 4c1e441f excluded.

## Traps — do not repeat these

- Earlier traps remain in [handoff-18.md](handoff-18.md) and its predecessors. Do not merge old-history pinned confirmation branches directly into main, rewrite main, or delete historical evidence refs.
- A helper-only argv test did not bind the real launcher log. The accepted dense regression executes the actual parser and logging block in a disposable untimed fixture.
- “Do not checkout” applies to the gf2 worker branch. A worker temporarily removed the owned comparator helper's pinned checkout; this would break fresh clones. The comparator helper was restored before its preparation commit.
- Help-only `perf annotate` checks do not establish behavior on actual perf data. Both the flagged and simplified commands work with retained data; the contradictory result is recorded. Validate the actual offline path and keep the gate history intact.
- After changing a renderer, regenerate projections that publish its current-checkout identity. Missing this caused DVB round-5 F1; the proposed fix is one generated hash row, not a new provenance mechanism.
- Re-key only c73's two baseline families. The old hold comment implied c73 owned M4RI too; the issue graph places that comparison in `50f0bd42`, and the queue comment now names it.
- Repeating counters while sampling hot symbols once repeats the DVB interval defect. Dense preparation collects both in all nine repetitions and logs failures before exiting.
- Perf event names are host-specific. Read-only listing did not expose the proposed LLC names; dense preparation uses listed generic cache events with matching labels.
- Do not overwrite the harness owner's M4RI-inclusive validation/smoke records with the baseline subset. c73 has its own named records; the original evidence remains.
- Preparation-only code review can report missing measurement deliverables. These findings remain open until the authorized window and interpretation; do not invent a passing review or request closure early.

## Open questions needing invoker input

- Approve DVB round 6: commit the existing generator's one-line current-renderer hash update and rerun code review?
  - Context: round 5 corrected annotation commands but missed regenerating the document that names their current hash; the generated diff changes no samples or measured source pins.
  - Options: approve the one-line regeneration, or keep the DVB job held.
  - Recommendation: approve. The concrete generated diff was inspected before asking.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [progress](progress.json), [worker brief](worker-brief.md), [measurement contract](measurement-contract.md).
- [Dense harness review](reviews/e1f9a78f-r7.md), [dense baseline review](reviews/c73ffa25-preparation.md), [DVB latest review](reviews/9fb40c83-r5.md).
- [Dense preparation](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-baseline-preparation.md), [DVB preparation](../c04dd4ac-zen3-shifts-and-permutations/dvb-profile-r4-preparation.md).
- [Queue](bench-window/queue.tsv), [window runner](bench-window/run-window.sh); runtime orchestration log `.agents/bench-window/window.log`.
- DVB generator: `dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-profile-provenance.py`; input `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-dynamic-profile`; output sibling `v4-r2-dynamic-profile-provenance.md`.
- Pinned out-of-wave trees remain as recorded in handoff-18; foreign `agent-02b8137c-run` is untouched. Raw session audit files under `/tmp/gf2-wave3-session24` are optional diagnostics, not durable evidence dependencies.

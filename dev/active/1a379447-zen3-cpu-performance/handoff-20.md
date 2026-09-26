# Handoff — Zen 3 CPU performance (1a379447) — session 25

**Date:** 2026-09-26
**Session number:** 25
**Prior handoffs:** Ordered list in [progress.json](progress.json), through [handoff-19.md](handoff-19.md).

## Current state

- Epic `1a379447`: tracker `backlog`, assigned to `agent:jit-execution-lead`; execution wave 3 remains in progress. User requests handoff after the current rework; no further work starts in this session.
- The two wave stories remain open. Their unfinished descendants are recorded in `session_scope`, now including tracked coverage bug `9f0e385e`.
- No active workers. The DVB work lease was released after review; query current claims before resuming other leaves.
- `gf2-bench-window-20260927.timer` is armed for **September 27, 02:00 Europe/Helsinki** (September 26, 23:00 UTC). Twelve pending jobs total an estimated 220 minutes. DVB is enabled; M4RI and later-wave independent confirmations remain held.
- No approval questions remain open. Progress: [progress.json](progress.json).

## What just happened

- User approved DVB round 6. Regenerated the provenance document with its existing generator; commit `42ce1503e` changes one renderer-hash row. A second regeneration is byte-identical.
- Round-6 code gate accepted that repair and flagged the report's historical/current annotation wording. User approved round 7; commit `599dc6111` cites the current launcher's declared command instead of repeating its flags.
- Round-7 code gate `547264f9-2ade-48ea-a194-368dab241b20` passes at `97436ebc8`. Its one pre-existing coverage-label advisory is tracked as `9f0e385e`; `c04dd4ac` depends on that bug. No completed criterion or coverage label was changed.
- Restored the 12-minute DVB fresh-profile row. Its clean worker remains `agent-9fb40c83-r4` at `71df9c69a`; main-only follow-up changes are report/provenance/tracker material. No measurement behavior or build artifact changed in this session.
- [Round-7 lead review](reviews/9fb40c83-r7.md) records passing code preparation and the still-unmet timed profile requirement. The DVB issue remains open; research review must wait for actual evidence.
- No measurements, host locks, perf sampling or benchmark-window environment overrides ran in this session.

## What to do next

- [ ] Let the approved 02:00 window run. Verify timer/service outcome and each campaign's authoritative log, terminal status, checkpoints, raw samples and acceptance verdict; scheduling is not evidence of execution.
- [ ] Preserve the pinned queued trees from handoff-19: dense baseline `815010276`, DVB `71df9c69a`, residual BMI2 `3c08c3494`, ISA-L `8f2875d8c`, logical candidates `2d33ac5ca`.
- [ ] Collect and review DVB's fresh `v4-r3-dynamic-profile`, produce its intervals and provenance, and run research review after the report is complete. Do not replace the accepted paired receipt with profile observations.
- [ ] Complete c73's baseline receipt/profile/portfolio requirements after its three queued jobs. Its code gate remains failed on those missing deliverables, as recorded in handoff-19.
- [ ] Resolve coverage bug `9f0e385e` before closing `c04dd4ac`: establish the canonical target of the completed breakdown's `satisfies:REQ-06`; obtain scope authorization before changing a coverage label if needed. No investigation or repair of that bug started here.
- [ ] Follow the wave DAG and sequential merged-tree CI discipline in handoff-19. Subsequent 02:00 overnight windows are authorized within this wave, but none beyond September 27 is armed. Keep wave 3 until both stories finish.

## Traps — do not repeat these

- All unresolved traps in [handoff-19.md](handoff-19.md) and its predecessors remain binding.
- A generated renderer-hash correction does not address surrounding report wording. Check both projections and prose references when a command changes; the round-6 gate caught the latter independently.
- The annotation help-only diagnosis is contradicted by successful actual-data checks in both command forms. Preserve that observation from the prior reviews; do not present it as a reproduced runtime failure.
- Passing the preparation code gate does not complete the profile issue. Its repeated measured attribution and research gate remain pending.
- The coverage advisory is tracked, not waived or silently relabeled. The namespace defines contribution to a container criterion; resolve the actual owning container before judging or editing the label.

## Open questions needing invoker input

None. User-approved rounds 6 and 7 are complete. Any later scope change or repair beyond the authorized rounds requires its own decision under the execution policy.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [progress](progress.json), [prior handoff](handoff-19.md), [worker brief](worker-brief.md), [measurement contract](measurement-contract.md).
- [DVB round-7 review](reviews/9fb40c83-r7.md), [DVB report](../c04dd4ac-zen3-shifts-and-permutations/dvb-interleave-profile.md), [provenance](../../bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-dynamic-profile-provenance.md).
- Coverage advisory: `jit issue show 9f0e385e`; affected breakdown `16560579`.
- [Queue](bench-window/queue.tsv), [window runner](bench-window/run-window.sh); runtime orchestration log `.agents/bench-window/window.log`. Individual campaign logs remain authoritative.

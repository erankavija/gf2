# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 41

**Date:** 2026-10-22
**Session number:** 41
**Prior handoffs:** handoff.md and handoff-2.md through handoff-36.md in this directory.

## Current state

- Epic: `1a379447` — state: backlog.
- Wave in progress: 4; existing wave plan unchanged.
- Immediate children: 32 done, 2 backlog, 1 rejected. The remaining chain includes story `ed3d490e` and its in-progress outcome task `6230e0b6`.
- Active assignments: epic agent:jit-execution-lead; `6230e0b6` agent:worker (since 2026-10-21). Reclaim attempts failed before writing.
- Open escalations: report location and story criteria from [handoff-36](handoff-36.md).
- Progress file: `progress.json` in this directory; resolved using the compatible CLI.

## What just happened

- Two read-only agents assessed HPC work (gpt-6.1-sol, high) and CLI compatibility (gpt-6.1-sol, medium).
- Confirmed doc-review F5 already has its correction on main; F1 remains an explicit criterion/location conflict. No gates rerun or implementation changes.
- Identified an existing compatible CLI: `/home/vkaskivuo/Projects/just-in-time/target/jit-stale-child-test-cache/debug/jit`, build e2da055a. Its doc listing, gate listing and doc directory resolution work without installation or configuration changes.
- HPC assessment: batch route selection lacks worker-count context; pooled decoding and simulation orchestration are the callers that must convey execution concurrency. A worker-aware policy belongs in gf2-coding and its canonical offline selectors. Existing measured multicore losses motivate `d06a99ed`; no new measurements were run.

## What to do next

- [ ] Resolve the two scope questions below before dispatching dependent work.
- [ ] Use the compatible CLI explicitly; the installed CLI cannot load current configuration.
- [ ] After the location ruling, apply it and rerun the required gates for `6230e0b6`; F5 requires no further edit.
- [ ] If approved, bring `d06a99ed` into the story and resolve its worker-context API before implementation. Check actual pool width at execution, not just decoder construction: pooled decoding supports another pool width on later calls.
- [ ] Complete the story checkpoint, then shared-runner task `e5f8b3ae` and scorecard `1362381c` in dependency order.

## Traps — do not repeat these

Prior unresolved traps remain in [handoff-36](handoff-36.md).

- Installed `jit` 1.0.0 and ordinary sibling debug/release builds reject the current `docs-mechanical` and gate `auto` fields. Do not delete configuration fields to fit an older executable; use the compatible existing binary above.
- Do not reimplement doc-review F5: `c64a3efb9` already qualifies the comparison statement to the before-after families.
- Do not infer that a renewed execution request approves the unresolved criterion changes. The prior handoff explicitly records those choices as open.
- Initial claim failed opening `.git/jit/locks/claims.lock` under the filesystem sandbox. Use an approved elevated command for Git metadata writes; do not mistake it for a stale-claim conflict.

## Open questions needing invoker input

- Approve bringing `d06a99ed` into the story to fix worker-aware route selection, accepting the unavailable AFF3CT cells as recorded limits and retaining fixed-wave investigation `f768c925` outside this epic? Recommended HPC-focused scope; it preserves the production adoption requirement. Exact criterion amendments must be explicit before closure.
- Approve amending `6230e0b6` REQ-01 and moving just its outcome report to `dev/active/1a379447-zen3-cpu-performance/6230e0b6/findings.md`, updating links and registration? Recommended narrow resolution, preserving the project-wide layout rule. Alternative from the prior handoff: broaden that rule to story-owned directories.

## Reference artefacts

- [Prior open decisions](handoff-36.md)
- [Decoder outcome record](6230e0b6/findings.md)
- [Generated decoder results](../../bench_results/ed3d490e/tables.md)
- JIT items: `1a379447`, `6230e0b6`, `ed3d490e`, `d06a99ed`, `e5f8b3ae`, `1362381c`.

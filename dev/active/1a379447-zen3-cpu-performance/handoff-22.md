# Handoff — Zen 3 CPU performance (1a379447) — session 26

**Date:** 2026-09-28T11:22:36Z
**Session number:** 26
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-21.md` in this directory. Read their unresolved traps in order.

## Current state

- Epic `1a379447` remains in progress. Current wave **3 of 7**; the invoker ends this session explicitly. Do not advance into held later-wave work.
- Wave-3 story `c04dd4ac` is **DONE**, with all seven gates passed. Story `2037941f` remains open behind `50f0bd42 -> 6e87c436 -> 2ad3a3e0`.
- `50f0bd42` is assigned to agent:jit-execution-lead, in progress, with preparation CI passed. Its code/research/assembly gates await completed measurement and outcome. No current-wave advisory lease remains; reacquire before resuming.
- Both escalations from handoff-21 were explicitly approved and applied. No open invoker question.
- No workers, builds, reviews, or measurements are running. Main and the M4RI worktree are clean.
- **Scheduled timer remains active:** `gf2-bench-window-20260929.timer`, September 29 at **02:00 Europe/Helsinki**, September 28 **23:00 UTC**. Verified at this handoff. Ending the session does not cancel it.
- The M4RI pilot is the sole unfinished active queue job, key **`53a5bb9b75c30e58`**. Its pinned tree is `.agents/worktrees/agent-50f0bd42`, branch `worktree-agent-50f0bd42`, HEAD **`458f38afe7b33f7260bbdeca5a96c768d3f80b36`**. Do not merge, rebase, reclaim or edit that tree before the window.
- `progress.json` records this state. Pre-handoff main HEAD is `1417ea822`.

## What just happened

- User approved both the exact shift-story background correction and the M4RI unavailable-row companion amendment.
- Applied the background correction at `e5f006b4b`; every other description byte and all criteria/gates remain unchanged. The approved correction artifact records the ruling.
- Shift story documentation review `d7ed6fd2`, research review `a81574d2`, and holistic review `262b7be8` pass without findings. Code review `5ffbbf6f`, CI, assembly and repository checks also pass. Final review `reviews/c04dd4ac-r2.md` maps all criteria and all prior findings. Story closed at `21dc9f7a3`.
- M4RI worker applied the approved v2 amendment at `5d96d19a1`, then completed the same-class wording sweep at `458f38afe`. Original v1 prose is preserved byte for byte; prior receipts are untouched.
- Final v2 addendum SHA-256: `7e088574182c627b9d9858fd74bd11c5f7774bb0d6fa5d57b4565e53ee89fac2`. Its measured pilot JSON differs from the original only in freeze timestamp. No qualified shapes, sampling rule, resolution ceiling or protocol schema changed.
- Final Rust 1.95 release build, 50 harness tests, semantic oracles and shared zero-window staged smoke pass. Projected pilot is eight qualified cells with exactly 24 pairs each. The three predeclared unqualified shapes appear only in the zero-sample/zero-comparison companion. Pilot and confirmation-shaped projections agree. Producing closure has 235 committed clean inputs.
- Merged the final amendment, passed full main CI, linked its artifacts, activated the reviewed queue entry at `3d010ac50`, and verified the timer. All older active queue keys already have completion markers. GF256/LDPC later-wave rows remain held.
- Lead waited without running builds or measurements. The user then requested this handoff and session end. The scheduled M4RI pilot has not started at handoff time.

## What to do next

- [ ] Respect the session stop. On a new invocation, read this handoff and progress, then inspect the timer and authoritative window log before doing any build or measurement work.
- [ ] After the timer runs, inspect `.agents/bench-window/window.log`, `53a5bb9b75c30e58.out` and the job's own execution log. A zero launcher exit alone does not prove complete measurement.
- [ ] Canonical output in the pinned tree: `dev/bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot`. Stage: `target/e1f9a78f-campaigns/v4-r1-2037941f-dense-matvec-vs-m4ri`. Preserve all raw receipts, checkpoints, logs, companion and spent ledger first, including ignored input snapshots required by their digests.
- [ ] Verify eight qualified cells at 24 pairs each and three companion exclusions at zero samples/comparisons; reproduce acceptance and check actual runtime paths and adapter costs. Keep fresh whole-consumer and retained-input operation boundaries separate.
- [ ] Apply the fixed M4RI resolution ceiling **0.040**. If exceeded, record terminal resolution-insufficient; never widen margins. Otherwise use the canonical freezer, fresh 24-pair confirmation and the family's own append-only P-20 ledger. Standing authorization permits subsequent **02:00 Helsinki** windows needed for wave 3; freeze and review preparation before queueing.
- [ ] Finish `50f0bd42` outcome and gates, then `6e87c436`, `2ad3a3e0`, and story `2037941f`. The dense portfolio has no fusion candidate; do not invent a prototype or relabel the established fused route.
- [ ] Keep main stable during each AI gate. Serialize heavy builds and full CI. For receipt-bearing merges, regenerate the schema receipt-count projection from the merged index before commit and clean-tree CI.
- [ ] Refresh current registered document asset hashes after linked-source changes. Preserve cumulative finding tables. `c04dd4ac` is closed; its round count remains 2. `50f0bd42` has one scoped preparation repair recorded.

## Traps — do not repeat these

- **Do not re-ask for either handoff-21 approval.** The user explicitly said “both approved”; the exact background correction and companion amendment are applied.
- **Do not stop the timer when ending the interactive session.** The user requested a handoff/session end, not cancellation of the separately approved scheduled benchmark.
- **Do not leave generic receipt prose contradicting the companion.** The first v2 patch missed decision-interval, sampling and final evidence-inventory sentences. The final sweep distinguishes runtime-discovered unavailable measured cells inside `receipt.json` from predeclared exclusions in `unavailable-rows.tsv`, including current harness prose.
- **Do not repin/rebuild after each individual wording match.** Complete the whole relevant prose sweep first, then update the digest and validation once. The final v2 digest above is the readiness authority.
- **Do not read historical v1 evidence as claiming v2.** Baseline preparation and conformance links now target the preserved v1 addendum; current M4RI preparation targets v2.
- **Do not use `jit claim status` without an agent identity.** It errors rather than reporting lease state. `jit claim list --json` needs permission for its `.git` lock even though it is read-only.
- All campaign, P-20, immutable snapshot, cache, stale narrative and asset-hash traps from [handoff-21](handoff-21.md) and earlier remain in force. In particular, ISA-PILOT-6 is confined to its exact terminal receipt and does not authorize six-pair M4RI work.

## Open questions needing invoker input

None. Session ended at the invoker's request; the scheduled timer remains authorized.

## Reference artefacts

- Epic and issues: `jit issue show 1a379447`, `jit issue show 50f0bd42`; progress is `progress.json` in this directory.
- Shift closure: [final review](reviews/c04dd4ac-r2.md), [pitfall audit](reviews/c04dd4ac-pitfall-audit.md), [publication](../c04dd4ac-zen3-shifts-and-permutations/publication-findings.md).
- M4RI: [preparation](../50f0bd42/preparation.md), [approved amendment](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-amendment-v2.md), [canonical v2](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md), [preserved v1](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md).
- Queue and runner: `bench-window/queue.tsv`, `bench-window/run-window.sh`. Logs live under `.agents/bench-window`; each campaign's own execution log is authoritative.
- Scratch audits and gate summaries: `/tmp/gf2-wave3-session26/`. Durable JIT gate records retain the full findings.
- Sol xhigh handled the final M4RI preparation. No Astra was used; configured gate model remains Terra. All worker branches named in handoff-21 are merged, but preserve the M4RI timing tree and all out-of-wave/foreign pinned trees.

# Handoff — Zen 3 CPU performance (1a379447) — session 26

**Date:** 2026-09-28
**Session number:** 26
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-20.md` in this directory; all unresolved traps remain in force.

## Current state

- Epic `1a379447` remains in progress; current wave **3 of 7**, restricted by the invoker to stories `2037941f` and `c04dd4ac` and their prerequisites. Do not advance the wave or run held GF256/LDPC work.
- Wave-3 stories remain open. Shift publication `a0812b83` is DONE. Dense descendants `50f0bd42`, `6e87c436`, `2ad3a3e0` remain incomplete.
- `c04dd4ac` has CI, code, assembly and repository gates passed. Documentation gate fails pending the background correction below; research and holistic gates have not run.
- Active lease: `c04dd4ac`, agent:jit-execution-lead, acquired 2026-09-28 00:05Z, expires 02:05Z. Other ancient leases belong to unrelated agents; leave them alone. Assignment on `50f0bd42` remains lead-owned; reacquire a lease on resume.
- Two open escalations: M4RI companion-table amendment and the story background correction. Both have concrete committed drafts and were asked through asynchronous questions; neither has a response yet.
- No active workers, builds, gates, timed runs, or future timer. Main is clean after this handoff commit. Progress remains in `progress.json`.

## What just happened

- Session completions: `9f0e385e`, `9fb40c83`, `65c0e13d`, `bc091474`, `a1ad6d4e`, `fcb04d66`, `00dd43c3`, `c73ffa25`, `a0812b83`.
- DVB round 8 completed with canonical uncertainty intervals; its profile remains not-material. ISA-L ends resolution-insufficient under the explicitly approved receipt-specific ISA-PILOT-6 exception. No widening of margins or new six-pair campaigns is authorized.
- Logical unrolls select no candidate; BitSlice has no production-copy signal. Logical integration is a verified no-change result with profile provenance repaired.
- The authorized September 28 02:00 Helsinki window completed both jobs: dense profile key `479f388792038140` and residual confirmation key `d01d0731883044f8`, ending 23:04:29Z September 27. Both have done markers and complete own logs.
- `00dd43c3` confirmation has six cells with 24 pairs, all Pass, qualifies=true. The frozen rule retains BMI2. Outcome and raw snapshots merged; code/research gates pass; no production edit was needed.
- `c73ffa25` dense profile has six cases with nine passes. No distinct fusion fits the frozen one-row full-count boundary. Both baseline receipts and profile are committed, all gates pass, no prototype is selected.
- `a0812b83` publication has deterministic tables, counts, intervals, source pins and completed dispositions. All four gates pass (code `0ec59744`, doc `29a58ab3`, research `9d556616`). Lead review is `reviews/a0812b83-r1.md`.
- `c04dd4ac` code `369860dc` failed stale investigation claims. Rework `e77bbdb22`, merged `3cc62baff`, fixes investigation and feasibility prose and regenerates only prose-source digest rows. Code `5ffbbf6f` passes. Story pitfall reconciliation is committed.
- Story doc `228fc432` fails F1 stale registered asset hashes and F2 stale issue background. F1 is repaired: publication/shift profile cite the stable `85fc5ff4` review instead of volatile progress; generator updated, CI passes at `3d3fd00b6`; canonical doc re-registration and direct SHA checks pass. F2 is the pending exact description amendment. Rework count is 2, with F2 awaiting permission to finish this round.
- M4RI preparation `2b75c6462` is merged and full CI passes. Rust 1.95 release build, 50 harness tests, semantic oracles and zero-window staged smoke pass. Projected pilot is exactly eight qualified cells with 24 pairs each. Frozen addendum and held queue row remain unchanged.
- M4RI strict JSON cannot represent its three predeclared unqualified shapes without a protocol change or false runtime arm. Proposed companion generator is implemented and tested on pilot and synthetic confirmation addenda. It records the existing rows, reasons, zero samples/comparisons and frozen prose digest. Draft is not adopted.

## What to do next

- [ ] Read the invoker's response to both pending questions. Do not treat silence or the older approvals as approval of these new amendments.
- [ ] If the story background correction is approved, apply exactly the draft sentence through `jit issue update --description-file`; preserve every other description byte/criterion/gate. This completes the already counted round 2. Record the ruling separately in progress.
- [ ] Verify registered local asset hashes after any linked-source edits using `jit doc add` again for the affected document. Do not pin a changing progress file as stable review evidence.
- [ ] Rerun `c04dd4ac` doc-review, then research-review and holistic-review sequentially on stable main. Read each complete result, preserve cumulative F1/F2 resolution tables, and finish the lead review before DONE. Any additional rework beyond round 2 needs explicit permission.
- [ ] If M4RI amendment is approved, finish it on the pinned `agent-50f0bd42` worktree: retain original frozen bytes/history, add the approved versioned amendment, update its identity/projection and producing-input snapshot, rebuild and repeat focused untimed checks. Do not change protocol v4/schema or statistical rules.
- [ ] Queue only the exact reviewed M4RI pilot after that preparation: issue `50f0bd42`, tree `.agents/worktrees/agent-50f0bd42`, 25 minutes, command `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-dense-harness.sh window --family 2037941f-dense-matvec-vs-m4ri --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/dense-matvec-vs-m4ri.json --run-id v4-r1 --m4ri`.
- [ ] Standing authorization covers subsequent 02:00 Europe/Helsinki windows needed for wave 3. No next timer is scheduled. September 29 02:00 Helsinki is September 28 23:00UTC. Use canonical queue/window runner and verify timer. No active-session timing or locks.
- [ ] After pilot: preserve raw evidence and spent ledger first; fixed M4RI resolution ceiling is 0.040. Never widen margins. If eligible, canonical fresh confirmation uses 24 pairs and its own P-20 budget; otherwise preserve terminal resolution-insufficient. No fusion candidate exists.
- [ ] Continue `50f0bd42 -> 6e87c436 -> 2ad3a3e0 -> 2037941f`, then finish wave 3 only.

## Traps — do not repeat these

- **Do not infer unavailable rows fit strict receipt JSON.** The runner only creates unavailable rows from runtime core-arm failure; unqualified M4RI shapes fail before execution. The prepared companion needs the explicit frozen-wording amendment.
- **Do not use pilot-only transcription validation for the companion.** A derived confirmation changes roles/resolution/description. The repaired generator uses canonical family validation and excludes overlap with measured cells; both paths are checked untimed.
- **Do not assert ledgers remain at genesis forever.** Completed baseline ledgers are nonempty; the repaired harness test checks existence and distinct ownership across lifecycle.
- **Do not publish measured medians with only observed ranges.** The publication now reuses canonical order-statistic intervals and derives coverage. At 24 pairs, show enough precision to avoid rounding coverage to exactly one. No new statistics framework was added.
- **Do not make semantic assertions depend on narrative wording.** A brittle retention sentence assertion was removed; acceptance and the frozen rule govern retention.
- **Do not link directly to `.jit/issues` storage.** `jit doc add` rejects that path as aliasing its data root. Use the issue ID and canonical graph query. A shell batch initially continued into a gate after registration failed; use dependency-aware error handling.
- **Do not mistake zero link errors for fresh registered hashes.** The tracker link check passed while four content hashes were stale. Re-register after source changes and independently compare local hashes. The repaired story has 20 matching local asset hashes; publication has 18.
- **Do not use mutable progress as the amendment-review authority.** The publication now cites `reviews/85fc5ff4-r1.md`; a progress update no longer invalidates its registered asset.
- **Do not restrict stale sweeps to `future`/`pending`.** Investigation said `unused`, `lack materiality`, `dispatch is only`, and retained a scalar-only integration proposal. Code re-review confirms closure; issue background itself remains pending explicit permission.
- **Do not reinterpret “description changes require approval.”** Even the one stale background sentence is covered by the invoked skill. Draft it concretely, then ask.
- Preserve raw whitespace and source snapshots. Receipt-bearing merges regenerate the schema receipt-count projection from the merged index before commit/CI. See prior handoffs for campaign, P-20, cache and timing traps.

## Open questions needing invoker input

- Approve `50f0bd42`'s generated `unavailable-rows.tsv` beside each receipt instead of inside `receipt.json`? Recommended: approve the prepared amendment. Three exclusions, their reasons and zero spending remain; measured cells, 24-pair rules and 0.040 ceiling are unchanged.
- Approve `c04dd4ac`'s exact background sentence replacement? Recommended: approve the current-route correction in the draft. Criteria, gates and implementation scope stay unchanged.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: [progress.json](progress.json).
- Shift review: [publication closure](reviews/a0812b83-r1.md), [story pitfalls](reviews/c04dd4ac-pitfall-audit.md), [background draft](../c04dd4ac-zen3-shifts-and-permutations/background-amendment-draft.md).
- M4RI: [preparation](50f0bd42/preparation.md), [amendment draft](50f0bd42/unavailable-rows-amendment-draft.md), [example](50f0bd42/unavailable-rows.example.tsv).
- Accepted outcomes: [BMI2](../00dd43c3/confirmation-outcome.md), [dense baseline](c73ffa25/outcome.md), [logical no-change](fcb04d66/no-change-outcome.md), [BitSlice](a1ad6d4e/outcome.md).
- Worker branches fully merged: `worktree-agent-a0812b83` through `781db5419`, `worktree-agent-c04dd4ac-r1` through `e77bbdb22`, `worktree-agent-50f0bd42` through `2b75c6462`. M4RI tree is the pending timing pin; keep it. Reclaim other completed trees only through the skill script, preserving `target` caches and with no builds active.
- Out-of-wave pinned GF256/LDPC trees and foreign `agent-02b8137c-run` remain untouched. See handoff-20 and earlier pins; never reuse `agent-c04dd4ac` (GF256), which differs from the documentation repair tree `agent-c04dd4ac-r1`.
- Scratch review results and audit logs: `/tmp/gf2-wave3-session26/`; durable gates are under `.jit/gate-runs/`. No Astra was used. Sol high handled documentation; Sol xhigh handled M4RI. Configured gate model remains Terra.

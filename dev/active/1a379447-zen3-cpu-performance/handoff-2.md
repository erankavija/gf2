# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 2

**Date:** 2026-09-07
**Session number:** 2
**Prior handoffs:** [session 1](handoff.md)

## Current state

- Epic: `1a379447` — backlog (dependency-blocked container).
- Wave in progress: wave 1 of 7; `f547c394` remains in_progress.
- Children summary: 0 done, 1 in_progress, 20 backlog/ready, 0 rejected.
- Active claims: no active lease; the lead released its f547c394 lease at the stop. Issue assignment remains `agent:jit-execution-lead`.
- Open escalations: none. Rework attempt 2 is interrupted by the user, not a third failed review.
- Worker checkout: `.agents/worktrees/agent-f547c394`, branch `worktree-agent-f547c394`, preserved at `cf2e7270`; not merged into main. Independent test checkout `.agents/worktrees/agent-f547c394-identity-tests` is already merged but retained until wave closure.
- Main is at the recorded R2 failure/rework state. Full CI passed before R2; code review fails R2 F1; doc review has only the earlier pass; research review has only the historical citation failure despite its approved metadata repair.
- Latest instruction: the user subsequently says **“stop here”**, superseding the request to finish wave 1 before stopping. Both active workers are stopped; no further implementation, tests or gates run after the stop.

- Invocation history: waves 1 and 2 → finish wave 1 then handoff → immediate stop. Wave 2 is not dispatched.
- Progress file: [progress.json](progress.json).

## What just happened

- Resumed the planned epic with Sol for protocol rework, Terra for independent contract audits, and Luna for independent checkpoint tests and wave-2 preflight.
- Applied the user's “Repair” approval: removed eight obsolete comparator citations from the protocol issue and added existing registry-key mentions to six survey descriptions. All first-two-wave citation tier-1 checks pass.
- Added the user's Git-provenance rule to canonical `@/inv/behavioral-evidence-validity` in `.jit/invariants.toml`; regenerated `AGENTS.md` with `jit project render --name invariants`. Updated the measurement contract and protocol issue criteria under the same explicit authorization.
- Replaced Git-critical evidence verification with immutable content snapshots of producing code/build inputs and protocol artifacts; shared checkpoint semantics ignore informational revision changes. Missing bytes and content mismatches reject.
- Published separate pilot and confirmation smoke receipts, preserving raw samples, append-only logs and checkpoints. Both resume over two sessions. The twelve-core confirmation cell remains unavailable under the six-core CCX1 affinity, so the receipt does not qualify for production selection. These are protocol pipeline checks, not gf2 speedup claims.
- Found and fixed ignored nested snapshot lockfiles; verified every producing input in committed exports. Added a real runner regression crossing an unrelated JIT commit and unrelated dirty documentation without repeating completed cells.
- Merged-tree full CI passes: 5,949 workspace tests, zero failures, 248 tier-skipped tests, plus tuning/campaign/baked/composer checks, lint and formatting.
- Independent review R2 found acceptance bound only the addendum to campaign-start; dispatched the second rework for complete frozen facts, plan and receipt binding. The rework remains unfinished at the user stop; [R2](reviews/f547c394-r2.md) is the latest formal verdict.

## What to do next

- [ ] Resume the **same second rework** of `f547c394` only on a new invocation. Read the full R2 verdict and cumulative findings; no third rework has been consumed.
- [ ] Inspect preserved worker changes at `cf2e7270` and the red-test commit `41f1de43`. `CampaignFacts` is shared by runner and acceptance; it checks direct receipt projections, saved plan and checkpoint identity. The seven self-consistent replacement cases pass under the fix according to the worker, but final suite and receipt verification are outstanding.
- [ ] Finish fixture repairs and unavailable-cell coverage. Last worker report: protocol suite 23/24, with decoder-quality fixture changing results after checkpoint publication; fixture ordering was being corrected. The unavailable checkpoint check is now before status dispatch (lead verified), with its regression added. No final suite verdict has been received.
- [ ] Audit canonical checkpoint validation reuse before requesting the final review: the in-progress evaluator explicitly parses unit JSON/schema/digests while `journal` owns checkpoint validation. `CheckpointStore::resume` can have recovery effects; do not replace read-only acceptance with mutating recovery. Decide whether the shared library needs a read-only validation entry point rather than a parallel parser. This is an observed implementation-review concern, not a completed blocker finding.
- [ ] Run focused suite, formatting and strict clippy; re-evaluate the existing committed pilot and confirmation with the corrected acceptance tool and record validator content identity. Keep frozen logs and raw samples unchanged; export verification must include every snapshot byte.
- [ ] Fold the lead-preserve commit into a reviewed final-form worker commit before integration; do not merge the interrupted preservation commit as final work. Keep a salvage reference as needed. Require raw pre-edit/final audits and the cumulative resolution table.
- [ ] Leak-check, integrate final work, and run full CI on the clean merged tree before another main commit. Link final evidence and evaluate code, doc and research gates individually, with an explicit lead lease. Finish six-tier review and mark done only if every gate and criterion passes.
- [ ] On wave-1 completion: update progress to wave 2, validate the DAG, release leases, and reclaim both wave-1 worktrees through the canonical reclaim script. Then honor the next invocation's scope.
- [ ] Begin wave 2 only on a subsequent invocation; load `progress.json`, verify all eight current issue states and dependencies, and re-read the approved plan/measurement contract and frozen protocol.
- [ ] Use isolated worktrees through `dispatch-worker-worktree.sh`, anchored to clean current main. Lead owns all `.jit` mutations and document linking.
- [ ] Suggested first group: Sol `c077a88b` (LDPC), Terra `1d0da41f` (YMM dispatch), Luna `eda07788` (shifts/permutations). Remaining issues: `04b85d10`, `6c6b09b1`, `26465e6c`, `c7113c5a`, `6fb89a3c`; allocate Sol to harder polynomial/transpose surveys and Terra to profiling/byte-field/reductions, adjusting by observed difficulty.
- [ ] Research issues require real adapters, pinned builds, frozen family addenda, correctness evidence and bounded current-code baseline receipts. The explorer template's findings-document-only default does not override their explicit criteria. Production optimization remains excluded from survey tasks.
- [ ] Reuse existing comparator/BCH/conversion harnesses and shared protocol support. Coordinate central module/schema/build-manifest edits; serialize timed phases under the existing host lock, and finish builds before measurement.
- [ ] After each sequential merge, run the configured full CI gate before another main commit or merge; then evaluate review gates individually and apply the six-tier lead review before closure.

## Traps — do not repeat these

- **The prior handoff's Git-object/ancestry demands are superseded.** Its F1/F2 instructions and matching traps conflict with the user's explicit amendment, recorded in [provenance clarification](../f547c394/provenance-clarification.md). Content snapshots and the premeasurement execution record establish provenance. Git IDs, availability, ancestry and whole-repository dirty status never control acceptance or resume. Missing content and distinct-pilot requirements remain.
- **Do not treat a single checked addendum as complete freezing.** R2 F1 demonstrates that protocol, contract, schema, producing inputs, plan, toolchain, settings and executable identities must correspond to the same recorded premeasurement facts. Reuse canonical typed comparisons and cover the category behaviorally.
- **Do not assume artifact files are tracked because local acceptance passes.** `.gitignore` ignores nested `Cargo.lock` files. Both producing snapshots initially omitted required locks from publication. Force-add exact required snapshot files and validate exported committed artifact trees.
- **Do not invoke `nextest` directly through the budget wrapper.** Luna omitted `cargo` and got `ionice: failed to execute nextest`; `cargo-nextest` was already installed. Correct command starts `./scripts/cargo-budget.sh --test cargo nextest`.
- **JIT leases need explicit identity here.** No default agent identity is configured. Use `--agent-id agent:jit-execution-lead` for acquisition and `JIT_AGENT_ID=agent:jit-execution-lead` for evaluation. Lease/claim reads can require writes to `.git` coordination locks; this sandbox requires escalation for that filesystem boundary.
- **Use `jit project render --name invariants`.** Positional `jit project render invariants` is rejected. Edit the canonical invariant registry before rendering its AGENTS projection.
- **A document-link warning returns nonzero.** `jit doc check-links` stopped a `set -e` gate chain on an unlinked historical review. Link the cited review to the issue and rerun; verify a subsequent checker actually started.
- **Use `jit claim status --issue <id>`.** A positional issue ID is rejected; `jit claim list --json` also reports active leases.
- Other unresolved session-1 traps remain in [the prior handoff](handoff.md#traps--do-not-repeat-these): commit worker steps early, no main commits during gate evaluation, lead-owned JIT writes, citation label/text agreement, cache/gate runtime expectations and behavior-preserving a835 extraction.

## Open questions needing invoker input

None. The immediate stop is explicit; further work needs a resumed invocation, not a clarification. If the final allowed rework later fails, apply the skill escalation policy with the full review history.

## Reference artefacts

- Epic: `jit issue show 1a379447`; protocol task: `jit issue show f547c394`.
- [Approved plan review](plan-review.md), [measurement contract](measurement-contract.md), [protocol](../f547c394/protocol.md), [schema](../f547c394/addendum.schema.json), [design](../f547c394/design.md), [producing inputs](../f547c394/producing-inputs.json).
- [Prior code-review R1](reviews/f547c394-r1.md), [R2](reviews/f547c394-r2.md), latest formal verdict. There is no passing final review yet.
- [Pilot receipt](../../bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/receipt.json), [confirmation receipt](../../bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/receipt.json), [launcher](../../bench_results/f547c394/run-smoke.sh). Each receipt directory contains raw checkpoints, execution/launcher logs, acceptance projections and portable input snapshots.
- [Canonical invariant registry](../../../.jit/invariants.toml); projection: [AGENTS.md](../../../AGENTS.md).
- Broader a835 campaign identity migration is tracked by `0ba493e1`; its pre-existing composer lockfile fragility is recorded in the protocol design and progress pitfalls. Neither justifies weakening this protocol's evidence rules.
- Repository-relative execution skill scripts: `.agents/skills/jit-execution-lead/scripts/`; do not follow the session-1 obsolete `~/.claude/skills/...` reference.

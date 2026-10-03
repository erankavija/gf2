# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 8

**Date:** 2026-09-01T05:01:22+03:00
**Session number:** 8
**Prior handoffs:** `dev/active/6dc81018-field-capability-dispatch/handoff.md`, `handoff-2.md`, `handoff-3.md`, `handoff-4.md`, `handoff-5.md`, `handoff-6.md`, `handoff-7.md`

## Current state

- Epic: `6dc81018` — state: in_progress
- Wave in progress: recovery wave 24; `5bdc9552` is escalated after the rework cap and `389aa4de` is the other inherited active root.
- Children summary: direct epic rollup is 10 done, 2 backlog; the open nested endgame remains `5bdc9552`, `389aa4de`, `06ba0418`, `eaae1b56`, `a83583e0`, and `dbd8787d`.
- Active claims: `5bdc9552` lease `459dffa1-6607-47fb-b52e-aa71416bc635`, agent `agent:codex`, acquired 2026-09-01T01:41:09Z, one-hour TTL; release or renew on resume.
- Open escalations: `5bdc9552` exceeded `MAX_REWORK_ATTEMPTS=2`; invoker choice is required before another correction.
- Progress file: `progress.json` in `dev/active/6dc81018-field-capability-dispatch/` reflects the above.

## What just happened

- Recovered the stale `5bdc9552` worktree onto current `main`; preserved the coherent selector-removal patch and ran the worker from the issue worktree.
- Removed the four `FiniteField` constants and the `Fp<P>` override, deleted proof substitutions/axioms, pinned `gf2_core::tuning` opaque in both extraction invocations, regenerated both proof trees, and repaired the obsolete gf2-algebra postprocessor.
- Independent review found the postprocessor matched every `CoreOps*` wrapper; rework 1 replaced it with a fail-closed exact ten-wrapper allowlist and five regression tests. A second regeneration is byte-identical.
- Invoker approved REQ-03A and design Amendment A2: the documented pre-existing `Packed5::to_raw_planes` projection is the sole additional generated declaration permitted beyond seam §3.3.
- `5bdc9552` merged at `41199824`; merged-tree cargo-ci passed with 5,212 tests, verify-lean passed, lake-build passed, and code-review initially found one present-tense documentation defect.
- Rework 2 replaced the affected criteria row with the live `gemm.winograd_min_dim` contract; the follow-up merged at `8e3554c5`. Cargo-ci and code-review then passed.
- Doc-review failed with three blocking stale-selector references: `strassen_threshold_results.md:62`, public `field/charpoly.rs:59`, and `benches/triangular.rs:4`. It also advises updating historical test comments in `field/ple.rs:3191` and `field/inverse.rs:1503`.
- Audited the remaining calibration chain. `389aa4de` has a concrete pre-measurement patch plan; `eaae1b56`, `a83583e0`, and `dbd8787d` require cumulative measured owner artifacts and remain serialized. `a83583e0` has a later design decision over three compile-time baked extents; do not address it before its dependencies close.
- Coordinated all main/JIT windows with Claude through `/home/vkaskivuo/Projects/forum-poc`; Claude moved intentional untracked review artifacts out of the repository before integration.

## What to do next

- [ ] Read the invoker response to the `5bdc9552` rework-cap escalation.
- [ ] If the invoker provides targeted guidance, record that decision and reset `rework_counts["5bdc9552"]`; fix all three blocking doc-review findings and both advisory test comments in one present-tense canonical-cutover sweep.
- [ ] Re-run the affected gate chain on a clean tree, require `jit gate status-all 5bdc9552` to pass, complete the six-tier lead review, mark `5bdc9552` done, release its lease, and reclaim its worktree through the skill script.
- [ ] Dispatch and close `06ba0418` using the completed read-only audit: update `220cab0b/design.md`, the epic classification, and the bounded live-prose sources while preserving dated historical artifacts.
- [ ] Resume `389aa4de` from a fresh clean worktree. Land its pre-measurement raw-sample/seed/route evidence patch before taking one full-host CCX1 lock for calibration plus composition.
- [ ] Continue the dependency chain exactly as tracked: `389aa4de` → `eaae1b56` → `a83583e0` → `dbd8787d`.

## Traps — do not repeat these

- **Do not treat dated benchmark evidence as permission for current sections to name removed APIs.** Code-review and doc-review separately rejected current criteria, introduction, heading, and rustdoc statements even though nearby raw evidence is historical. Rewrite live sections around profile fields; preserve only clearly measurement-time evidence.
- **Do not use a namespace-wide regex to axiomise extracted trait dictionaries.** The first postprocessor repair swallowed transparent `Neg` and would silently absorb new wrappers. The committed exact allowlist and exact-set/count tests are load-bearing.
- **Do not delete `Packed5::to_raw_planes` from the generated tree to make the old criterion literal.** It is reproducible pre-existing drift from Rust commit `9b78666c`; REQ-03A and design Amendment A2 explicitly permit exactly that projection and require byte-identical regeneration.
- **Do not merge while `git status -uall --porcelain` has intentional files.** The execution-lead leak guard still requires an empty tree. Claude moved the two owner artifacts out before the merge; repeat that coordination rather than treating them as a baseline.
- **Do not interpret a green code-review as satisfying doc-review.** Their attribution and policy sweeps differ; run the full current-prose grep before either gate.
- **Do not read the first merged-tree cargo-ci failure as a project failure.** The sandbox could not write `/run/user/1000/cargo-ci-slots/probe.lock`; the narrowly escalated identical-tree run passed. Use the approved `./scripts/cargo-ci.sh` escalation when that lock error recurs.
- **Do not restart silent cargo-ci or AI-review gates.** They buffer output; poll the existing session until the terminal summary.
- Prior handoff traps remain binding, especially strict format-2 ownership, no dirty-source measurement, one outer benchmark lock, and no commits during timed phases.

## Open questions needing invoker input

- Question: How should `5bdc9552` proceed after exhausting two rework retries?
  - Context: Implementation, proof, cargo, Lean, and code-review gates pass; doc-review found three blocking current-documentation references and two adjacent advisory comments.
  - Options: (A) provide targeted guidance authorizing one comprehensive present-tense sweep, which resets the counter; (B) take over those five documentation edits manually; (C) reject `5bdc9552`, which blocks `06ba0418` and epic completion.
  - Recommendation: A. The findings are precise, localized, and mechanically aligned with the repository invariants; fixing all five together avoids another fragmented stale-text round.

## Reference artefacts

- Epic: `jit issue show 6dc81018`
- Escalated issue: `jit issue show 5bdc9552`; `jit gate status 5bdc9552 doc-review --findings`
- Designs: `dev/active/7d7c647c/design.md`, `dev/active/7d824b2f/design.md`, `dev/active/3fa7c9d0/design.md`
- Epic execution state: `dev/active/6dc81018-field-capability-dispatch/progress.json`
- Proof-drift evidence: `dev/active/34d85cb9/findings.md`
- Calibration checklist: `dev/active/389aa4de/receipt-notes.md`
- Coordination forum: `/home/vkaskivuo/Projects/forum-poc`

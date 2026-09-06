# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 1

**Date:** 2026-09-07T00:20:00+03:00 (2026-09-06T21:20Z)
**Session number:** 1
**Prior handoffs:** none for this epic (plan review record: `dev/active/1a379447-zen3-cpu-performance/plan-review.md`)

## Current state

- Epic: `1a379447` — state: backlog (dependency-blocked container; assigned to `agent:jit-execution-lead` with `--assign-only`)
- Wave in progress: wave 1 of 7 (single issue `f547c394`)
- Children summary: 0 done, 1 in_progress (`f547c394`), 20 backlog, 0 rejected
- Active claims: `f547c394` assigned to `agent:worker` (assignment, no lease). Worker worktree `.agents/worktrees/agent-f547c394` on branch `worktree-agent-f547c394`, fully merged into main at `7a222264`, tree clean, no unmerged commits. Keep it for the rework; reclaim through `.agents/skills/jit-execution-lead/scripts/reclaim-worker-worktree.sh f547c394` only after the issue closes.
- Open escalations: one (stale `cites:` labels on `f547c394`; see Open questions). Rework round 1 is NOT dispatched: the invoker stopped the session before the rework message went out.
- Progress file: `progress.json` in this directory (wave plan for all 7 waves, dispatch record, rework count 1, surfaced pitfalls, escalation)

## What just happened

- Planned 7 dependency-depth waves over the 21 children created by the user-approved plan revision (`412dab33`, `4e08a061`); wave 1 = `f547c394` alone because every other child depends on it.
- Dispatched `f547c394` (implementation, Fable model) in a worktree anchored to `974a21f4`. The worker hit its usage limit after scaffolding six modules with nothing committed; resumed the same worker after the reset, with an instruction to commit early.
- Worker delivered eleven `jit:f547c394` commits: protocol document with a clause coverage table and rules P-01..P-22, JSON Schema for family addenda, paired-bootstrap A/B statistics, receipt acceptance tool, lock-wrapped protocol runner over the existing journal and checkpoint primitives, host/process mechanics lifted from the a83583e0 driver, 137 crate tests, and a two-session smoke receipt under `dev/bench_results/f547c394/`.
- Lead: leak check clean; merged `--no-ff` at `7a222264`; linked 11 documents (`4fda522e`); cargo-ci gate PASSED on the merged tree (`43310ff7`); recorded pitfalls (`740ebb86`).
- Review gates: code-review FAILED with two blocking findings (run `2421a450`); doc-review PASSED with no findings; research-review FAILED at its deterministic tier 1 (eight `cites:` labels without a matching citation in the issue text) before any AI review ran.
- Recorded the round-1 verdict at `reviews/f547c394-r1.md` and set `rework_counts.f547c394 = 1` (`b57a94e3`); the status string there says "dispatched", which is wrong — the rework was never sent.
- Answered an invoker question about single-core phases in cargo-ci: they are serial recompiles of gf2-core (104k lines, 2025 unit tests) under several feature and cfg configurations; sccache does not cache workspace crates across checkouts.

## What to do next

- [ ] Resolve the open question below (stale `cites:` labels on `f547c394`) with the invoker; research-review cannot pass until it is resolved, and the worker cannot fix it (issue labels and description are lead/invoker-owned).
- [ ] Dispatch rework round 1 to the `f547c394` worker (attempt 1 of 2). Use `rework-prompt-template.md`, paste `reviews/f547c394-r1.md` in full, and require the resolution table. Required changes: (1) `ArtifactPin::read_pinned` (`dev/tools/tuning-campaign-support/src/protocol.rs:185-215`) must never fall back to the working tree; an unavailable Git object or digest mismatch is an error-severity P-02/P-03 finding that rejects the receipt, and the corresponding note path in `src/receipt.rs:463` goes away. (2) Freezing must be proven, not declared: the receipt's addendum pin commit must contain the addendum with the pinned digest and be an ancestor of or equal to the receipt's source revision (`git merge-base --is-ancestor`); drop or redefine `frozen.at_commit` accordingly and update `protocol.md` "Roles and freezing" plus rows P-02/P-03; `effect.resolution_evidence` must name a committed pilot receipt (path and digest) distinct from the receipt under evaluation, verified by the tool. (3) Regenerate the smoke evidence as a pilot receipt followed by a confirmatory receipt that cites it; supersede `2026-09-06-f547c394-smoke/` (remove it and say so in the commit message; it is a pipeline proof with no performance claim). (4) Add fixtures for: missing Git object, digest mismatch at the pinned commit, addendum pin not an ancestor of the source revision, missing or self-referential resolution evidence. Keep protocol version 1: it has not been consumed by any family yet and the issue is not complete; state that in `design.md`.
- [ ] After rework: leak check, merge, cargo-ci gate, then evaluate `code-review`, `doc-review`, `research-review` individually (not `evaluate-all`, see Traps). Apply Tier 1.5 with the round-1 findings F1 and F2 from run `2421a450`.
- [ ] On PASS: `jit issue update f547c394 --state done`, commit, `jit graph downstream f547c394`, `jit validate`, reclaim the worktree, advance `current_wave` to 2.
- [ ] Wave 2 (eight issues, all `research` except `1d0da41f` implementation): `04b85d10`, `6c6b09b1`, `c077a88b`, `eda07788`, `c7113c5a`, `26465e6c`, `6fb89a3c`, `1d0da41f`. Dispatch in worktrees via `dispatch-worker-worktree.sh`; the comparator builds (AFF3CT, ISA-L, srsRAN, xdsopl, gf2x, libpopcnt, M4RI/M4RIE, Bitshuffle) are CPU-heavy and each survey needs the CCX1 lock for timed work, so stagger the timed phases. Model by difficulty: `c077a88b` (LDPC arms) and `c7113c5a` (polynomial baselines) are the hard ones.

## Traps — do not repeat these

- **Do NOT let a worker run long without a first commit.** The Fable worker hit its usage limit after ~40 minutes with six modules uncommitted; only the worktree on disk saved the work. Require a commit as soon as the scaffold compiles.
- **Do NOT commit on main while a gate evaluation is running, and do NOT use `jit gate evaluate-all` after HEAD has moved.** HEAD moved during the cargo-ci evaluation (doc-link commit `4fda522e`), so a later `evaluate-all` would re-run the 10-minute cargo-ci checker instead of reusing the pass. Evaluate the review gates individually by key.
- **Do NOT expect `jit doc add` to work from a worker worktree.** The tracker refuses state writes from linked checkouts; the lead links artifacts from the primary checkout and re-validates with `jit doc check-links --scope issue:<id>`. This is not permission laundering: it is jit's worktree write policy.
- **Do NOT accept a working-tree fallback when verifying pinned evidence.** The code reviewer rejects it (F1, `protocol.rs:185`): a pinned artifact is verified only against `git show <commit>:<path>`; an unavailable object is a rejection, never a note.
- **Do NOT let an addendum cite the receipt it governs as its own resolution evidence, or declare a freeze commit that predates the addendum.** Reviewer F2. The freeze is proven by the addendum pin commit being an ancestor of the measurement revision; resolution evidence is a separate committed pilot receipt.
- **Do NOT dispatch a research-review-gated issue with `cites:` labels its description does not mention.** The gate's tier 1 fails deterministically (no AI review is spent). `f547c394` inherited eight such labels from the pre-revision planning. Check every wave-2 issue's `cites:` labels against its text before dispatch; several survey tasks were re-scoped by `412dab33`.
- **Do NOT gate on main without expecting ~10 minutes of serial gf2-core recompiles.** cargo-ci rebuilds gf2-core under several feature/cfg sets; the main checkout is not seeded from the cache pool. Plan gate time accordingly.
- **Do NOT treat the a83583e0 driver refactor as evidence invalidation.** `dev/active/a83583e0/premeasurement-protocol.md` section 9 anticipates support extraction and pins committed evidence by executable digest; the driver's 22 unit tests and the campaign contracts still pass. Advisory only.

## Open questions needing invoker input

- Question: Remove the eight stale `cites:` labels from `f547c394` (`AlbrechtBard2026`, `Bitshuffle2026`, `Cassagne2019`, `GfComplete2026`, `GfTwoX2026`, `IsaL2026`, `Libpopcnt2026`, `Mula2018`), or mention those keys in its description?
  - Context: the plan revision moved the comparator survey out of `f547c394` into the family tasks, but the labels stayed; research-review's tier-1 label/text drift check fails on them before any content review.
  - Options: A) remove the eight labels (`jit issue update f547c394 --remove-label cites:<key>` or the label command the CLI provides); B) add a citation sentence to the description; C) leave and accept a permanently failing gate.
  - Recommendation: A. The protocol task builds on the statistics citations the worker registered inline (`Efron1979` and others, resolvable in the registry), not on the comparator projects. Removing the labels changes no criterion.

## Reference artefacts

- Epic: `jit issue show 1a379447`; measurement contract `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md`; plan review record `plan-review.md` (same directory)
- Progress file: `dev/active/1a379447-zen3-cpu-performance/progress.json`
- Round-1 verdict: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md`; code-review run `.jit/gate-runs/2421a450-5812-455c-8fe9-caa49294f2da/result.json`
- Worker deliverables: `dev/active/f547c394/{protocol.md,addendum.schema.json,addendum-protocol-smoke.json,design.md}`; `dev/tools/tuning-campaign-support/src/{abtest,host,process,protocol,receipt,schema}.rs` and `src/bin/{benchmark-ab-runner,benchmark-acceptance,ab-smoke-workload}.rs`; tests `tests/{protocol_contracts,host_process_contracts,schema_subset}.rs`
- Smoke evidence (to be superseded in rework): `dev/bench_results/f547c394/run-smoke.sh`, `dev/bench_results/f547c394/2026-09-06-f547c394-smoke/`
- Worker's final report: delivered in-session to the lead (not a file); its criterion table is reproduced in the round-1 verdict's evidence cites.
- Worktree protocol and scripts: `~/.claude/skills/jit-execution-lead/references/worktree-dispatch-protocol.md`, `.agents/skills/jit-execution-lead/scripts/`

# Handoff — Overhaul documentation for research adoption (fa787f85) — session 1

**Date:** 2026-10-02
**Session number:** 1
**Prior handoffs:** none

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`. It cannot move to in_progress until its dependencies are done.
- Wave in progress: wave 5 of 16, not yet dispatched. Waves 1–4 (the contract story `3f29e945` and its five children) are done.
- Children summary for the subtree: 9 done, 12 ready (wave 5), 101 backlog, 1 rejected.
- Active claims: none held by the lead. Two stale leases from 2026-09-15 (`3ea122df`, `92385645`, `agent:work-*`) belong to other work.
- Open escalations: none.
- Progress file: `progress.json` in this directory holds the wave plan, statuses, rework counts, `surfaced_pitfalls`, `traps` and `notes`.

## What just happened

- `495807a3` (register documentation invariants): its work was already on main from a prior session. The first review failed on a stale draft record; one sonnet rework; done.
- `34adff85` (rustdoc and doctest steps in cargo-ci): the prior branch was cut from a two-month-old base and was discarded. The task was redone with opus on current main. code-review failed because doctests skipped `cargo-budget.sh --test`; one rework; done.
- The jit 1.0 upgrade (just-in-time adb5e9df) made unqualified `satisfies:`/`cites:` labels dangling, so every repo-validate gate failed (985 errors). With owner approval, cross-epic tasks `f023b88b` and `0dc52df8` (filed this session in epic `86b9c719`) migrated the labels. Owner rulings are recorded as f023b88b DEC-01/02: membership owner wins; orphans removed. Both done, and `jit validate` is clean.
- Main checkout edit to `contrib/gates/doc-review-prompt.md` (archive exemption removed): committed as-is on owner instruction (1868d7926).
- `56378e82` (CLAUDE.md symlink, AGENTS.md scope): passed first try, net −6 lines.
- `dcd38c45` (invariant-grounded doc-review prompt): passed first try. The owner chose to keep the two REQ-05 run reports in this directory. The profile-owned prompt was folded into sim-research 1.2.4 (aca9d60e4).
- `8f61d6de` (docs-mechanical gate): the owner decided citations are checked on the permanent surface only (DEC-01). Three deliberate quotations are baselined in `8f61d6de-baseline-findings.md`. One rework round; done.
- OD-08: `docs-mechanical` was added to every open overhaul issue (114).
- Owner decision: `cargo-ci` and `code-review` were removed from 25 purely documentation issues; the list is in `progress.json` `notes`. No reviewer prompt changed.
- `3f29e945` story checkpoint: doc-review and holistic-review failed twice on stale planning-brief statements (sweep owner, manifest owner, invariant ownership, inherited README issue, finished breakdown). The brief was fixed, then all six gates passed; done.

## What to do next

- [ ] Dispatch wave 5 in two sub-waves. 5a runs in parallel worktrees:
  - one worker on the AGENTS.md lane, serially `9b2886a7` → `871b401d` → `a82d2347`;
  - `a0a29512` (opus);
  - `b6fdb815` (opus);
  - `a24b2af7` (opus);
  - `41e1d47d` (opus);
  - each of the roadmap mappers `e85b8edf`, `4bdcd65a`, `60652fe4`, `875914b3` (sonnet).

  5b is `12907582`, after `a0a29512` merges, since both edit crate rustdoc.
- [ ] Tracker-writing tasks (roadmap mappers file issues; `41e1d47d` rejects or labels issues and edits the brief): the worker commits a reviewed `jit` command script plus its record, and the lead runs the script on main after merge.
- [ ] `9b2886a7` and `871b401d` register invariants. If the worktree's `jit project render` refuses to write, the lead renders the projection on main after merge and commits it under the issue scope.
- [ ] Before the `ffc35b8c` story and epic gates, reconcile `surfaced_pitfalls` in `progress.json`. In particular, about 40 source comments cite nonexistent `CLAUDE.md §` sections; that subject belongs to REQ-17 (`12907582` and the sweep).
- [ ] After each wave: leak check, reclaim worktrees, update `progress.json`.

## Traps — do not repeat these

- **Do NOT edit profile-owned files in place and stop.** `contrib/gates/*-prompt.md` and the gate and config contributions declared in `packages/sim-research/manifest.toml` fail `jit validate` (`profile-ownership`). After the edit, run `jit profile capture --source packages/sim-research --destination packages/sim-research`, bump the manifest `version`, then run `jit profile upgrade --profile path:packages/sim-research`. Evidence: dcd38c45 broke validation until aca9d60e4.
- **Do NOT write pending or future-tense text in records.** "removes it later", "pending approval", and "once X exists" each failed doc-review (`@/inv/current-state-scope`): 495807a3 draft, 8f61d6de baseline header. The latter came from a lead instruction. Brief workers to state only the current contract.
- **Do NOT reuse old worker branches without checking their base.** The `worktree-agent-ab61dd5f…` branch was based on f9224650a (August). Always dispatch through `dispatch-worker-worktree.sh`, never Agent `isolation: "worktree"`.
- **Do NOT dispatch from a dirty main.** Gate runs modify `.jit/issues`, so commit gate state before calling the dispatch script, which refuses otherwise.
- **Do NOT tell workers to use `jit doc dir <task>` for record placement.** It resolves to `dev/active/<task-id>`. The active-layout contract puts records under `dev/active/fa787f85-documentation-overhaul/`.
- **Do NOT expect workers to write the tracker.** Worktree jit refuses state writes, so workers cannot run `jit doc add`, gate evaluation, or issue creation. The lead links artifacts and runs generated scripts.
- **Do NOT fix only the cited line in planning records.** Story review rounds 1–2 each surfaced a different stale sentence in `fa787f85-planning-brief.md`. Sweep the whole document for the defect class before re-gating.
- **Do NOT treat a holistic finding about a failed or unrun peer gate as a content defect.** Run the container's executable gates before `holistic-review`.
- **Keep commit subjects ≤71 characters**, merge and state commits included. One lead commit needed an amend.
- **cargo-ci budget is tight on a cold cache.** A cold run took 875 s against the 900 s timeout; the release doctest step takes about 215 s. A cargo-ci failure by timeout on a loaded host is not a code defect; rerun warm before reworking.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show fa787f85`; resolved structure: `jit graph tree fa787f85`
- Plan: `dev/active/fa787f85-documentation-overhaul/plan.md` (shared contracts, decision table)
- Brief: `dev/active/fa787f85-documentation-overhaul/fa787f85-planning-brief.md` (D-01..D-56)
- Investigation: `dev/active/fa787f85-documentation-overhaul/investigation.md`
- Progress: `dev/active/fa787f85-documentation-overhaul/progress.json`
- Gate scripts: `contrib/gates/docs-mechanical.py`, `contrib/gates/doc-review-prompt.md`
- Label migration record: `dev/active/86b9c719-quality-documentation-tech-debt/label-migration.md`

# Handoff — Overhaul documentation for research adoption (fa787f85) — session 13

**Date:** 2026-10-08
**Session number:** 13
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6), `handoff-6.md` (session 7), `handoff-7.md` (session 8), `handoff-8.md` (session 9), `handoff-9.md` (session 10), `handoff-10.md` (session 11), `handoff-11.md` (session 12)

## Current state

- Epic: `fa787f85`, state done; its six gates passed on their first run.
- Waves: 16 of 16 closed. Every child is done or rejected.
- Active claims: none.
- Open escalations: two, below and in `progress.json` `escalations` (the first) and this handoff (the second).
- Worktrees of this epic: none. Main is clean at the handoff commit and unpushed.
- Progress file: `progress.json` in this directory; completion report: `completion-report.md`.

## What just happened

- Owner rulings, each a DEC on its issue: `9451498f` REQ-02 amended; `a0ec34f2` REQ-03 amended and two research READMEs retained by commit pin; `d6ebc4ec` absent areas leave the policy with 88 references to archived mirrors commit-pinned, baseline at `contrib/gates/docs-mechanical-baseline.md`; epic DEC-02 to DEC-05 (held entries, baseline rows, pinned narrative, policy drift in `AGENTS.md`); the `epic:documentation-overhaul` label removed from eleven issues outside the graph.
- Done, every gate passed: `9451498f`, `a0ec34f2`, `4979271f`, `d6ebc4ec`, `d0b1545e`, `c423c7aa`, `c852d9b2`, `af84619e`.
- A read of the tree against the twenty criteria ran before the epic gates; its findings closed as `4979271f`, `d0b1545e`, `c423c7aa`, the epic decisions and the linking of 36 files of this directory.
- `packages/jit-default` re-minted at 1.0.3. `migration/` holds the completed manifest; the checker and its self-test are absent.
- Filed under `b4b4b9ee`: `bdf8e2e5`, `720f5f74`, `727677b4`, `aed7e749`; `835f34f0` and `c2663ce9` joined it.

## What to do next

- [ ] Settle the two open questions below.
- [ ] Tell the lead of epic 1a379447 that three review-gate runs of `c852d9b2` (23:00:20Z to 23:05:43Z on 2026-10-07, no build) overlapped the benchmark window that timed `bfc2ceab` key `d84574bfa3613f71` and `63bad95d` keys `fc1788421f764063` and `3e2cd5609ecb53ad`; `.agents/bench-window/window.log` holds the load at each job start.
- [ ] Delete the merged `worktree-agent-*` branches of this epic.
- [ ] `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/` holds scratch of ended sessions (two clones and a campaign stage, about 9 GB on tmpfs); the owner removes them.

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-11.md` remain in force. New this session:

- **Do NOT echo a subject's length and commit in the same chain without a guard.** Two 72-character subjects reached main; one was reworded within the minute, `62cd9a3eb` stays (open question). Use `[ ${#s} -le 71 ] && git commit`.
- **Do NOT run any gate through 01:55 to window end, review gates included.** The hold of `handoff-8.md` names builds and gates; three AI reviews ran inside the window of 2026-10-07. Read `date` before each gate batch after midnight.
- **Do NOT name a verified commit that lacks the outputs its commands read.** `c852d9b2` failed both reviews; the outputs go in a commit, then the record names that commit.
- **Do NOT commit an output of `jit doc check-links`.** It follows untracked `target/doc`, tracker state and network reachability; no clean checkout reproduces it.
- **Do NOT commit a final report produced while the tool it retires still exists.** `af84619e` failed on `53/55`; the complete report is the tool of the commit checked run on the tree without it.
- **Do NOT leave a worker's bare short ids in a commit body or a removed path in its record.** Rebuild the unmerged branch (soft reset to the merge base, typed messages) before the merge; three units passed after it.
- **Do NOT point an open issue's reference into an archive without a commit pin.** The 97bf0879 preview turned ineligible (`unpreservable-layout`) until the milestone's deck reference was pinned.
- **Do NOT brief "relink references to archive destinations" for absent source areas.** The planner maps an archived mirror back to its source area; the remedy is the commit pin (`d6ebc4ec` DEC-01).
- **`jit dep add` rejects an edge that leaves the graph unreduced.** Use `--reduce`, and read the result: a chain of `&&` stops at the first rejection.
- **Removing scratch of other sessions under `/tmp` is denied to the lead.** Report sizes and leave the removal to the owner.

## Open questions needing invoker input

- Question: is the 72-character subject of `62cd9a3eb` reworded?
  - Context: unpushed main; the commit lies under the merges of `c423c7aa`, `c852d9b2` and `af84619e`, whose records name later commit ids (`c852d9b2-link-verification.md`, `af84619e-checker-retirement.md`).
  - Options: leave it and record the exception; reword by `rebase -r` and update the commit ids the two records name.
  - Recommendation: leave it unless main is rewritten for another reason before the push.
- Question: is the epic's own directory archived now?
  - Context: `jit archive container fa787f85 --json` on the done epic; before completion the preview listed moves with pending deletions, copies and retained files. `dev/index.md`, the zen3 session records and `.jit/config.toml` history cite paths of this directory.
  - Options: run a pre-flight per `handoff-8.md` (every pending deletion and move against consumers and digests), then execute twice; keep the directory active until epic 1a379447 no longer cites it.
  - Recommendation: the pre-flight in an attended session.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Completion report: `completion-report.md`; progress: `progress.json`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md`
- Unit records of this session: `a0ec34f2-terminal-rows.md`, `4979271f-design-entry-move.md`, `d6ebc4ec-policy-narrowing.md`, `d0b1545e-deck-copy-removal.md`, `c852d9b2-link-verification.md`, `af84619e-checker-retirement.md`
- Link baseline: `contrib/gates/docs-mechanical-baseline.md`; manifest: `migration/manifest.toml`

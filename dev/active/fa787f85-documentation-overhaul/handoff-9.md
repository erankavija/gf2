# Handoff — Overhaul documentation for research adoption (fa787f85) — session 10

**Date:** 2026-10-06
**Session number:** 10
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6), `handoff-6.md` (session 7), `handoff-7.md` (session 8), `handoff-8.md` (session 9)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 11 of 16 (`616e1d7c` is its one open unit); waves 12 to 16 hold the other open units.
- Children summary for the wave plan: 119 done, 3 in_progress, 5 ready, 6 backlog, 2 rejected.
- Active claims (`agent:worker`, no worker running): `616e1d7c`, `7dbcde0c`, `b23c364a`.
- Open escalations: five (`5ec5dcc1`, `7dbcde0c`, `616e1d7c`, `b23c364a`, the zen3 cluster); below and in `progress.json` `escalations`.
- Worktrees of this epic, each holding an unmerged branch: `agent-616e1d7c` (tip `f29db982d`), `agent-7dbcde0c` (`48795ea3d`), `agent-b23c364a` (`0a8712dd8`). Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner ruling: `dev/active/389aa4de/` goes under the 86b9c719 directory (DEC-02 on `616e1d7c`).
- Done, all gates passed: `fdb998ec` (root README, 201 to 71 lines, first gate run), `0b8f09f3`, `8466f5ae`, `b2bb1065`, `ec655592` (first gate run after a pre-gate fix), `0a0ece44` (one rework), `0c5aa3b7` (two reworks), `b959ffe8` (`dev/index.md`, 117 to 49 lines, first gate run), `fca578b1` (one rework), `e1f262a7` (one lead record fix), `2596b143` (first gate run).
- `dev/plans/` is absent. Archives 97bf0879 and 026fc832 rerun twice each after the moves; `archive-rerun-results.md` holds the rows.
- `fca578b1` audited the permanent surface with three verification passes; its record `fca578b1-permanent-surface-audit.md` lists each fix with its invariant address. Thirteen issues filed under `b4b4b9ee` track the policy and code divergences it found; `progress.json` `created_during_execution` lists them.
- `2596b143` census: `2596b143-final-example-census.md` compares the final corpus with the pre-removal baseline.
- Filed in this epic: `8466f5ae`, `0b8f09f3` (both done), `b23c364a` (decoupling of entry 4e732b56).
- `decoupling-findings.md`: a read-only investigation of the eight entries left flat, with consumers, pins, CI checks and a task proposal (T1 to T4, amendment A1).
- `198aafa3` is unblocked: `4c1e441f`, `ad2a6a58`, `f63a2464` and `2037941f` are done.
- Returned partial and unmerged: `616e1d7c` (rework 2 of 2), `7dbcde0c`, `b23c364a`.

## What to do next

- [ ] Apply the owner's answers to the five open questions, then:
  - `616e1d7c`: under option A, `git apply target/616e1d7c-receipt-links.patch` in its worktree, regenerate `616e1d7c-verify.txt`, drop the record's unresolved-links paragraph, bring the branch to main, merge, run `target/616e1d7c-relink.sh`, rerun the 6dc81018 archive twice and record both runs, regenerate `96cea1b9-archive-rows.txt`, then gate. Its record must state the committed tracker reference and name no uncommitted script.
  - `7dbcde0c` and `b23c364a`: merge both before gating either; `dev/active/fd9d5416/receipt-protocol.md:7,154` must read `../4e732b56/` after both merges. Remove the ignored `__pycache__` directories of the two source entries on main.
- [ ] Under option A of the zen3 question: file T2 to T4 from `decoupling-findings.md`, amend `602652bf` (A1) and the criteria of `f79bcba3`, `198aafa3`, `7039d680`, `602652bf` for the hazards listed there, then dispatch in the dependency order of the proposal. `f79bcba3` sets the six manifest rows whose source `jit:b2e09d41` removed.
- [ ] `f6148d1e` and `f694f59a` after `616e1d7c`; then `9451498f`, `d6ebc4ec`, `c852d9b2`, `af84619e`.
- [ ] Before the epic gate, reconcile `surfaced_pitfalls`: the test-written receipt at the top level of `dev/bench_results` against REQ-14, and the `AGENTS.md` rows of the audit against REQ-02 and REQ-17.

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-8.md` remain in force. New this session:

- **Do NOT accept "historical" as a ground for an old path.** `0c5aa3b7` and `0a0ece44` failed review on citations left in handoffs, progress and breakdown JSON of other epics. Accepted grounds: a digest pin at file:line, a marker-backed archive, receipts and `inputs/` snapshots, tool output at a named commit, manifest schema fields, tracker state.
- **Do NOT repoint a path inside a preregistration because no digest of it is committed.** The lead had `e1f262a7` edit `design.md:517` and reverted it before merge; `plan.md` `relocation-protocol` fixes those bytes.
- **Do NOT replace a Rustdoc path citation with a vague phrase.** "the dataset README" failed `0c5aa3b7` doc-review; the README stays beside its dataset and the Rust lines keep their bytes.
- **Do NOT relink a commit-pinned tracker reference.** It names the file at its commit under the old path; re-pinning to the move commit changes which bytes it names.
- **Do NOT leave tracker asset metadata stale.** After repointing links inside a tracker-referenced document, rescan it with `jit doc add` (same type and label); `0c5aa3b7` failed on it.
- **Do NOT scope a move unit with a shallow glob.** `0c5aa3b7` missed a file; use `git ls-files -- <dir> | grep` and leave no manifest row of the directory pending.
- **Do NOT let a record name an uncommitted relink script or an outstanding relink.** `e1f262a7` failed doc-review on it; state the committed tracker reference.
- **Do NOT list a divergence in a record without a fix or a tracking issue.** `fca578b1` failed both reviews under `no-deferred-defects`.
- **Do NOT write a criterion that asserts a build without running it.** `b23c364a` REQ-04 came from an investigation draft; the crate does not build at the baseline.
- **Do NOT `cd` into a worktree, even for a read.** The lead's shell moved once; a worker's working directory was reset to another worktree mid-task. Use `git -C` and absolute `cd` inside each command.
- **A subagent's own helpers can hand back to the lead.** Three verification reports of `fca578b1` arrived here; forward the output file paths to the worker.
- **`jit gate evaluate <id> cargo-ci` without `--force` reran the suite at a new HEAD** while a worker built; it passed, but gate order matters under load.
- **The session clock drifts from memory.** Read `date` before timing a gate batch against the benchmark window.

## Open questions needing invoker input

- Question: does `5ec5dcc1` wait for `02b8137c`, or is its REQ-01 amended as for `f79bcba3`?
  - Context: `02b8137c` (epic b8206228) is in progress; branch `worktree-agent-02b8137c-run` carries one unmerged commit on the entry.
  - Options: wait; amend so the entry of an open owner stays and is reported.
  - Recommendation: the amendment.
- Question: is `7dbcde0c` REQ-04 amended, and is the `run.sh` edit accepted under REQ-05?
  - Context: after the move the renderer test results are identical and the synthetic receipt differs on two lines that print three run-time-observed paths; `d1b4f85e/run.sh:28` resolves the root with `git rev-parse --show-toplevel`.
  - Options: amend REQ-04 to identity apart from those paths and accept the edit; drop the branch.
  - Recommendation: the amendment.
- Question: how are the two unresolved links of `616e1d7c` closed?
  - Context: they sit in the archive copy of the live receipt `dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md`; editing the copy alone makes the 6dc81018 plan ineligible.
  - Options: A, change both files' link targets (no digest pins the receipt); B, amend REQ-03 to exempt byte copies of receipts.
  - Recommendation: A.
- Question: is `b23c364a` REQ-04 amended?
  - Context: `gf2-side` fails with `E0432` on `gf2_coding::bch::BchEncoder` at the baseline and after the move alike; its path dependencies resolve.
  - Options: amend to path-dependency resolution with the build result unchanged, and file the port as debt; port the harness first.
  - Recommendation: the amendment.
- Question: how does the zen3 survey-harness cluster proceed?
  - Context: `decoupling-findings.md`, sections "Owner decisions" (D1 to D5) and "Hazards in the sibling issues": `cargo-ci` runs `--check` on two producing-closure generators that name these directories, against the criteria of `f79bcba3` and `602652bf`.
  - Options: A, file T2 to T4 and amend the four move units; B, a named exception that keeps the entries flat until epic 1a379447 is terminal, with epic REQ-11 amended.
  - Recommendation: A, with D3 and D4 by regeneration where no build is needed and D5 keeping the literal.
- Question: does the `AGENTS.md` mission sentence keep its audit edit?
  - Context: `fca578b1` removed "high-performance" under `@/inv/no-marketing`; restoring it adds a line to a 200-line file.
  - Recommendation: no strong preference.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, rework_counts, created_during_execution)
- Worker brief for move units: `wave11-worker-brief.md`
- Decoupling facts and task proposal: `decoupling-findings.md`
- Audit and census records: `fca578b1-permanent-surface-audit.md`, `2596b143-final-example-census.md`
- Unit records of this session: `0c5aa3b7-sim-studies-notes.md`, `0a0ece44-stray-entries.md`, `b2bb1065-bench-reports.md`, `ec655592-dev-plans-removal.md`, `e1f262a7-design-move.md`
- Archive evidence: `archive-rerun-results.md`

# Handoff — Overhaul documentation for research adoption (fa787f85) — session 11

**Date:** 2026-10-07
**Session number:** 11
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6), `handoff-6.md` (session 7), `handoff-7.md` (session 8), `handoff-8.md` (session 9), `handoff-9.md` (session 10)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 12 of 16; its nine open units wait on owner rulings. Waves 13 to 16 hold `9451498f`, `d6ebc4ec`, `c852d9b2`, `af84619e`.
- Children summary for the wave plan: 123 done, 9 blocked, 4 pending, 2 rejected.
- Active claims (`agent:worker`, no worker running): `f79bcba3`, `f6148d1e`, `f694f59a`.
- Open escalations: seven, listed under "Open questions needing invoker input" and in `progress.json` `escalations`.
- Worktree branches of this epic hold no unmerged commit; `f6148d1e` and `f694f59a` have no commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner rulings applied: the six questions of `handoff-9.md` (all recommendations), "move the zen3 entries now, the parallel session conforms", and `616e1d7c` REQ-04 (DEC-04).
- Done, all gates passed: `616e1d7c` (two reworks after the ruling), `7dbcde0c`, `b23c364a`, `5ec5dcc1`.
- Filed in this epic: `b5b336e5` (bc091474), `2e18b37c` (f8dd4dde, a203a23c, 19513245), `cc060974` (00dd43c3, c04dd4ac). Criteria of `f79bcba3`, `602652bf`, `198aafa3`, `7039d680` amended under option A of `decoupling-findings.md`.
- Zen3 lane, seven units in one worktree, merged on main in four merges: 27 entries live under `dev/active/1a379447-zen3-cpu-performance/`. Unit records: `f79bcba3-zen3-move.md`, `b5b336e5-bc091474-move.md`, `602652bf-protocol-inputs-move.md`, `198aafa3-zen3-move.md`, `7039d680-fixture-entries-move.md`, `2e18b37c-zen3-move.md`, `cc060974-residual-shift-move.md`.
- Tracker references of the moved entries name the new paths; `jit validate` exits 0. The 6dc81018 archive reran twice after each `616e1d7c` merge (`archive-rerun-results.md`).
- `b5b336e5`: `cargo-ci`, `docs-mechanical` and `code-review` pass; `doc-review` fails on the issue description (question 4).
- `f6148d1e`, `f694f59a`: the worker stopped before any commit (questions 5 and 6).
- Twelve defects found by the units are tracked under `b4b4b9ee`; `progress.json` `created_during_execution` lists them. `5b3e7c81` tracks the move of the held 02b8137c entry.
- Session records of epic 1a379447 repointed after that session ended (merge `a9b51f208`).

## What to do next

- [ ] Apply the owner's answers, then per unit: rerun `doc-review` and `code-review` (`cargo-ci` and `docs-mechanical` ran on the merged tree; read `jit gate status-all <id>` first). Order: `f79bcba3`, `b5b336e5`, `602652bf`, `198aafa3`, `7039d680`, `2e18b37c`, `cc060974`.
- [ ] Before gating `198aafa3`, `7039d680`, `2e18b37c`, `cc060974`: regenerate each `<unit>-remaining.txt` with its script on main and confirm no `unclassified` line; the listings follow edits of other records.
- [ ] `f6148d1e`, `f694f59a` per the answers to questions 5 and 6.
- [ ] Then `9451498f`, `d6ebc4ec`, `c852d9b2`, `af84619e`. `migration/check.py` exits 0 and reports pending rows per disposition; `af84619e` REQ-01 needs every row terminal.
- [ ] Reclaim the merged worktrees `agent-zen3lane`, `agent-616e1d7c`, `agent-7dbcde0c`, `agent-5ec5dcc1` if they still exist; `agent-b23c364a` holds the uncommitted evidence of question 5 and stays until that question is settled.
- [ ] Before the epic gate, reconcile `surfaced_pitfalls`: the test-written receipt at the top level of `dev/bench_results` against REQ-14, the `AGENTS.md` rows of the audit against REQ-02 and REQ-17, the held flat entry 02b8137c against REQ-11 (question 7), and the baselined unresolved links against REQ-19 (question 1).

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-9.md` remain in force. New this session:

- **Do NOT gate a unit before linking its record and outputs.** `7dbcde0c`, `b23c364a` and `5ec5dcc1` failed both reviews on records without `jit doc add`. Link every file the worker names, then gate.
- **Do NOT let a relink script abort midway.** `jit doc add` refuses a path or captured asset beyond its depth budget and a non-UTF-8 document; three scripts stopped there and left references at absent paths, which failed `jit validate` and the `repo-validate` gate for every epic. Fall back to `--skip-scan`, and run `jit validate` after every relink. The generic pass of this session: for each unpinned reference whose path the tree lacks, add the counterpart path with the same type and label, then remove the stale reference.
- **Do NOT write a move unit's description with the path the unit removes.** `b5b336e5` doc-review failed on "The flat `dev/active/bc091474/` entry holds"; name the entry by issue id.
- **Do NOT put a volatile count in a record.** `5ec5dcc1` failed both reviews on "26 rows" (`single-source-prose`); cite the output that states it.
- **Do NOT accept a verifier output generated in a worker worktree.** The worktree's untracked `target/` files and stale `.jit` snapshot changed two counts of `616e1d7c-verify.txt`; regenerate on main after the merge and compare.
- **Do NOT key a verifier or listing on line numbers or counts of `progress.json` or a handoff.** The lead edits them every session; `616e1d7c-verify.py` failed on main for it.
- **Do NOT brief "update path literals" without the reproduction rule.** A literal a script writes into a record, or the text of a frozen document it reproduces, keeps its bytes; `552a0548d` broke six frozen addenda and `06fe749d3` reverted it. Such lines go to the owner (question 3).
- **Do NOT ask a worker for a full enumeration file.** `616e1d7c-citations.txt` arrived with 3126 lines; ask for an aggregate per class inside the existing output.
- **Do NOT run `cargo-ci` beside another session's CI.** `5ec5dcc1` failed twice on gf2-sim 15 s timeouts at load 49 and passed on the idle host; `fixture-leak` counts move the same way.
- **Do NOT edit files of another session's epic directory while it runs.** Its handoffs and reviews were repointed after it ended; `bench-window/queue.tsv` lines and its `progress.json` keep their bytes.
- **`jit issue claim` prints nothing and claims nothing for an issue with an unmet dependency.** The lane ran on the worktree; claim state followed the merges.
- **`2596b143-doctest.txt` is tool output at commit `cac8fa20e`.** The other lead asked for a refresh after the `llr-f64` removal; it stays frozen.

## Open questions needing invoker input

1. Question: do byte-fixed documents keep unresolved relative links after a move?
   - Context: `f79bcba3` REQ-05 and `602652bf` REQ-06 require both unchanged bytes and no unresolved link. Frozen addenda and protocols of 2037941f and f547c394 keep their bytes; their unresolved links are the rows of `contrib/gates/docs-mechanical-baseline.md`. Epic REQ-19 names unresolved links.
   - Options: amend the link clause to exempt byte-fixed documents and keep the baseline rows; change pinned bytes and the digests that cite them.
   - Recommendation: the amendment.
2. Question: is `f79bcba3` REQ-03 amended for `04b85d10/survey/gf2-side`?
   - Context: the crate fails with `E0432` on `BchEncoder` before and after the move alike; the port is tracked in `7ff06801`.
   - Options: amend to an unchanged build result, as `b23c364a` DEC-01; port first.
   - Recommendation: the amendment.
3. Question: do path literals that reproduce committed bytes keep the flat path?
   - Context: `198aafa3` REQ-04, `7039d680` REQ-05, `2e18b37c` REQ-04 and `cc060974` REQ-01 require updated or absent literals; the `progress.json` escalation lists each file:line, and `breakdown.json` of c04dd4ac keeps 14 flat paths.
   - Options: amend the four criteria to exempt text a script reproduces from a frozen document or committed record; change the literals and regenerate the committed bytes.
   - Recommendation: the amendment. `8163dacd` tracks the converse defect.
4. Question: may the opening sentence of `b5b336e5`, `2e18b37c` and `cc060974` be reworded?
   - Context: each names a flat path the unit removed; `b5b336e5` doc-review fails on it.
   - Options: reword to name the entry by issue id; leave and accept the finding.
   - Recommendation: reword.
5. Question: how do `f6148d1e` and `f694f59a` proceed?
   - Context: the byte-fixed campaign declarations of dbd8787d and a83583e0 name their protocol and producing manifest by flat path, and the extent validator and driver join those strings to the root; a trial move broke the `tuning-campaign-validator` CI step and one `tuning-campaign-support` test.
   - Options: A, a decoupling task in this epic that resolves those paths relative to the located declaration, which changes measurement tooling that epic 1a379447 campaigns run; B, hold both entries flat in the `5ec5dcc1` form with a tracked move.
   - Recommendation: A when no extent campaign is in flight, B to close the epic first.
6. Question: is REQ-07 of `f6148d1e` and `f694f59a` amended for tracker references?
   - Context: dbd8787d references name archive copies; a83583e0 references are commit-pinned to the flat path under `616e1d7c` DEC-01 so the 6dc81018 archive retains the entry.
   - Options: amend to "references keep their archive path or pin"; relink and accept re-collection.
   - Recommendation: the amendment.
7. Question: is epic REQ-11 amended for held flat entries?
   - Context: `dev/active/02b8137c/` stays flat while its issue is open (`5ec5dcc1` DEC-01, move tracked in `5b3e7c81`); option B of question 5 adds two more.
   - Options: amend REQ-11 to admit a flat entry with a tracked move issue; keep the epic open until each moves.
   - Recommendation: the amendment.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, rework_counts, created_during_execution)
- Worker brief for move units: `wave11-worker-brief.md`; decoupling facts: `decoupling-findings.md`
- Records of the completed units: `616e1d7c-regroup.md`, `7dbcde0c-bch-entries-move.md`, `b23c364a-baseline-survey-move.md`, `5ec5dcc1-held-entry.md`
- Archive evidence: `archive-rerun-results.md`; link baseline: `contrib/gates/docs-mechanical-baseline.md`
- Evidence of the stopped declaration move: worktree `agent-b23c364a`, `target/decl-return.md`

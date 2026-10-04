# Handoff — Overhaul documentation for research adoption (fa787f85) — session 8

**Date:** 2026-10-04
**Session number:** 8
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6), `handoff-6.md` (session 7)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 10 of 16. Its three tasks are done; the story gate of `ffc35b8c` is the one open item.
- Children summary for the wave plan: 98 done, 31 backlog, 2 rejected, 1 in_progress.
- Active claims: `ffc35b8c` (`agent:jit-execution-lead`, claimed 2026-10-04 for its gates).
- Open escalations: one, on `ffc35b8c` (decision identifiers; below and in `progress.json` `escalations`).
- Worktrees of this epic: none. Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner interview: `3fd3db5e` and `7dbcde0c` REQ-01 amended (DEC-01 on each: the overhaul edits the renderer and performs the move while `fd9d5416` is open); the `library-first-generality` exception of `bch/dvb_t2/mod.rs` recorded on `1a8f6acd`; `545b113f` split by crate.
- `3fd3db5e` done: the renderer and its test locate inputs through `document` of `dev/scripts/repository_files.py`. Code-review failed once on the baseline record's source commit; the record names `fde2218e3`.
- `545b113f` done in two rounds. Round 1 failed code-review on title lines the lead had workers restore. Round 2 merged each title into one orientation paragraph across every crate and `dev/tools`, with contracts under headings: 211 files, 859 insertions, 1659 deletions against `808f03aeb`. Record: `545b113f-module-docs.md`.
- `ffc35b8c` story gates, run 1: `cargo-ci`, `docs-mechanical`, `repo-validate`, `code-review`, `doc-review` passed; `holistic-review` failed on F1 (decision identifiers) and F2 (citation coverage of `554c2935`).
- `fa4939c3` filed under `ffc35b8c` for F2 and done on the first gate run: a read of 41,604 comment lines, 624 classified lines, five registered works, 20 lines with a new address, three corrected attributions. Record: `fa4939c3-comment-read.md`.
- Filed under `b4b4b9ee`: `9c7e6369` (the `HipStream` `Sync` safety comment attributes thread safety of `hipStreamSynchronize` to HIP documentation that states it for `hipStreamQuery`).
- `ffc35b8c` story gates, run 2: every gate passed except `holistic-review`, which fails on F1 alone; F2 is closed.
- Sweep completion record and citation map regenerated at `fa29c6a39` and after `fa4939c3`.

## What to do next

- [ ] Apply the owner's answer on decision identifiers, then re-run `holistic-review` on `ffc35b8c` (`jit gate evaluate ffc35b8c holistic-review --force`) and close the story.
- [ ] Wave 11, re-archive units (`83f9ce69`, `f29a9225`, `3759f995`, `42037c91`, `1ca94ec2`): six previews report blockers from tracker references that name absent paths. `jit archive container <epic> --json` lists them under `blockers`; the current files are in the epics' archive directories. Relink each reference first (lead, on main), then execute the archive on main, then dispatch the citation repointing.
- [ ] Wave 11, `f902240f`: the preview of `b7157be6` reports eligible with no blocker; REQ-01 still requires the marker check.
- [ ] Wave 11, regroup units (`4f161183`, `616e1d7c`, `eace5009`) and `103a792a`: worktree workers; tracker relinks run on main by the lead from a script the worker leaves uncommitted.
- [ ] Wave 11, `2596b143`: its REQ-01 records doctest execution time. Check whether that is a timed measurement for the overnight window before dispatch.
- [ ] `7dbcde0c` (wave 12) is unblocked by `3fd3db5e`; it waits on `eace5009`.
- [ ] Then waves 12 onward per `progress.json`.

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-6.md` remain in force. New this session:

- **Do NOT keep a title line above the orientation paragraph of a module doc.** The code reviewer reads title, blank line and overview as two paragraphs and fails REQ-01 (`545b113f`, run of 2026-10-04). The accepted form is one unheaded paragraph, then `#`-headed sections; trailing intra-doc link definitions are the one exception (`gf2-kernels-hip/src/host/mod.rs`).
- **Do NOT brief a Sonnet worker for a "reduce" task without the keep list.** `w545-kern` trimmed 57 compliant docs and dropped safety statements, reduction identities and a citation. Name what stays: safety, mathematics, reasons, feature lists, citations.
- **Do NOT let `git revert` write the subject.** Its `Revert "..."` form breaks the commit contract; `git reset --soft` and commit with a typed subject.
- **Do NOT write `target/` scratch files from two worktrees under one name.** Worktree `target/` is hardlinked from the cache pool, so `target/strip-comments.py` and return files of one worker appear in another's worktree. Name scratch files by unit.
- **Do NOT file an issue from a worker's claim.** `b4e878d3` was withdrawn before commit: the thread-count determinism tests of `permanent_bipedal3_parallel` exist in the ignored tier. A second claim (`extension.rs` `deg f >= 2`) was consistent with the code. Read the item first.
- **Do NOT commit on main while a review gate runs.** The reviewers judge HEAD; file issues and write the handoff after the batch returns.
- **Do NOT regenerate `030496bd-comment-census.txt` after `fa4939c3`.** `fa4939c3-comment-read.py` compares the census commit's `crates` tree with the commit read; regenerate both together.
- **Do NOT call the Workflow tool for a fan-out.** One empty call was made by mistake this session; it spawned nothing. Use Agent calls.

## Open questions needing invoker input

- Question: which form of decision identifier is canonical, `DEC-nn` or `D-nn`?
  - Context: `holistic-review` of `ffc35b8c` fails because `.jit/config.toml` `[item_kinds.decision]` has `id-pattern = "D-[0-9]+"` while 25 issues in six epics write `DEC-nn`; `jit item show ffc35b8c/DEC-01` and `93aed46a/DEC-01` do not resolve, and the sweep completion record cites them for the REQ-26 exceptions.
  - Options: widen the id-pattern in `.jit/config.toml` to accept `DEC-nn`; rename `DEC-nn` to `D-nn` in the Decisions sections of this epic's issues and in the records that cite them; rename in every epic.
  - Recommendation: the first option. It is one line, edits no description, and commit bodies that say `DEC-nn` stay true.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, created_during_execution)
- Sweep records: `030496bd-sweep-completion.md`, `545b113f-module-docs.md`, `fa4939c3-comment-read.md`, `554c2935-citation-map.md`
- Sweep worker brief: `ffc35b8c-sweep-brief-template.md`
- Archive previews: `archive-preview-results.md`

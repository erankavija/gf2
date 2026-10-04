# Handoff — Overhaul documentation for research adoption (fa787f85) — session 6

**Date:** 2026-10-04
**Session number:** 6
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 8 of 16 (the source-comment sweep under story `ffc35b8c`). One unit is open: `13dfe1a5`.
- Children summary for the wave plan: 90 done, 1 in_progress, 1 ready, 33 backlog, 2 rejected.
- Active claims (`agent:worker`, no worker running): `13dfe1a5`.
- Open escalations: one, on `13dfe1a5` (below and in `progress.json` `escalations`).
- Worktrees of this epic: none. Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Merged wave E: `eba21481` (`70fea53e9`) and `50201002` (`4e6327377`).
- Fix lane 5 (`39a68b205`): rework 1 of `13dfe1a5`, `c6aa0e84`, `f2064e90`.
- Gate batch at `751a92d10`: `8f451167` and `c6aa0e84` passed every gate; six units failed review.
- Fix lane 6 (`036156111`): rework of `13dfe1a5` and `f2064e90` (2 of 2) and of `72768e37`, `aa0558c1`, `eba21481`, `50201002` (1 of 2), with a planned-work audit over every tagged commit of each unit.
- Gate batch at `4293afd8d`: `f2064e90`, `aa0558c1`, `72768e37`, `eba21481`, `50201002` passed every gate. `13dfe1a5` passed `cargo-ci`, `docs-mechanical` and `doc-review` and failed `code-review`.
- `554c2935`: record rewritten to the current state (`2223efcc3`), output regenerated, all gates passed.
- Lead commits: `a747d0fdc` (routine name removed from `tests/channel_capacity.rs`), `docs(jit:eba21481)` (a mismatched Sage version key removed from three gf2-core bench headers).
- Done this session: `8f451167`, `c6aa0e84`, `f2064e90`, `aa0558c1`, `72768e37`, `eba21481`, `50201002`, `554c2935`.
- Filed under tech-debt epic `b4b4b9ee`: `17559b23`, `ee3bf65e` (worker defect reports; each issue's first criterion is to reproduce or refute).
- `git diff --shortstat 589c90016 HEAD -- crates dev/tools` gives the session's source line counts.

## What to do next

- [ ] Apply the owner's answer on `13dfe1a5`; after an approved rework, run workspace clippy, one forced `cargo-ci`, then its four gates.
- [ ] After any Rust comment edit: rerun the command in `554c2935-citation-map.md` and commit the output (line numbers shift).
- [ ] Wave 9, `030496bd`: the after census by `62f0d0e6-comment-census.py`, the justified pattern lines, and every issue of `progress.json` `created_during_execution` whose reason names planned work. Inputs are in the session 6 note of `progress.json`.
- [ ] Before the `ffc35b8c` story gate: reconcile `surfaced_pitfalls` against REQ-21 to REQ-28.
- [ ] Then waves 11 onward per `progress.json`. `3fd3db5e` waits on `fd9d5416` (in progress, another epic).

## Traps — do not repeat these

Traps of `handoff.md`, `handoff-2.md`, `handoff-3.md` and `handoff-4.md` remain in force. New this session:

- **Do NOT rely on a worker's final message alone.** It is truncated near 4,000 characters (`fixlane5`, `cite554`). Have the worker write the full return to `target/<name>-return.md` in its worktree and read that file.
- **Do NOT brief "delete name-echo docs" without the public-item rule.** `72768e37` and `aa0558c1` failed REQ-04 for deleted docs on `pub` items; `f2064e90` failed REQ-01 for echo docs on private items. Brief both: delete on private items; on a `pub` item write one line that states what the name and signature do not. gf2-core, gf2-sim and gf2-algebra set `#![warn(missing_docs)]`.
- **Do NOT restore a `pub` helper doc that paraphrases the body.** `13dfe1a5` code-review failed on four such lines at `4293afd8d` (`bench_mode`, `test_vectors_path`, `test_vectors_available`, `num_frames`).
- **Do NOT gate a unit whose commit bodies say `planned work: none` without the deleted-line audit.** Four units failed on statements naming tracker ids (`0d9cb8e3`, `6fb4abad`, `d48a3cfd`, `ad597ede`). The audit method is in the body of `612d4e93e`.
- **Do NOT leave a `dev/active/` path in a module doc, and do NOT replace it with an `@/issue/` address.** The first fails `no-dev-path-coupling` (`13dfe1a5`); reviewers class the second as planned work (`bfb37a21`). State what the artifact is.
- **Do NOT count a subject's length from `%h %s` output.** The hash is included; a 72-character subject needed an amend. Count `${#s}` of the subject string.
- **Do NOT `cd` into a worker worktree from the lead shell.** The session's working directory moves there; use `git -C <worktree>`.

## Open questions needing invoker input

- Question: approve a third rework of `13dfe1a5`, and which rule holds for `pub` items in `tests/`, `benches/` and `examples/`?
  - Context: the rework limit is reached. `code-review` fails REQ-01 on four one-line docs that paraphrase the body; the second rework restored them because `doc-review` of `72768e37` required purpose docs on `pub` test-support helpers. `doc-review` passes at the same commit.
  - Options: delete those docs and every other body-paraphrasing doc on `pub` items of the unit's files; keep purpose lines and narrow the helpers' visibility (a code change outside REQ-05, needs a DEC); reject the finding by an owner DEC.
  - Recommendation: the first option. Test targets of gf2-coding set no `missing_docs` lint.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, traps, created_during_execution)
- Sweep worker brief: `ffc35b8c-sweep-brief-template.md`
- Failed review of `13dfe1a5`: `jit gate status 13dfe1a5 code-review --all`
- Citation registry: `.jit/references.toml`; map: `554c2935-citation-map.md`

# Handoff — Overhaul documentation for research adoption (fa787f85) — session 7

**Date:** 2026-10-04
**Session number:** 7
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 10 of 16 (`545b113f`, then the story gate of `ffc35b8c`). Waves 8 and 9 are closed.
- Children summary for the wave plan: 95 done, 2 ready, 32 backlog, 2 rejected.
- Active claims: none.
- Open escalations: two, on `3fd3db5e` and on the `bch/dvb_t2/mod.rs` exception (below and in `progress.json` `escalations`).
- Worktrees of this epic: none. Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner interview on the open points; rulings are in `progress.json` `escalations` and as DEC-01 on `13dfe1a5`.
- `13dfe1a5` rework 3 (`1f3695200`, lead commit `f10058039`): 25 comment lines deleted, none added. `code-review` failed once on the collapsed `if` of `445b6169c`; the owner ruling of the same day was recorded on the issue as DEC-02 and the gate passed.
- `157c305c` pulled into the epic (`a57134b5d`): `FILLER_LLR` is a documented `pub const`; docs link to it. All gates passed.
- `4b8cebcf` filed and done (`88d55ba2a`, lead trim): `table_interpretation-amendment.md` supersedes the wrap statement; the note keeps its bytes.
- `3783c4c8` filed and done (`477348ffc`): `dvb_t2_matrices.rs` module doc is one sentence; 46 lines deleted, 2 added.
- `030496bd` done (`52ddb1191`, `01f3af686`): `030496bd-sweep-completion.md` with generated outputs; linked to `030496bd` and `ffc35b8c`. All gates passed on the first run.
- Filed under `b4b4b9ee`: `10a729d3` (planned work removed by `66858db0`; named by empty commit `1078dc1a5`).
- Filed under `ffc35b8c`: `545b113f` (module docs beyond the comment contract).
- Citation map regenerated after the wave 8 merges (`docs(jit:554c2935)`).
- Session `gf2-4e` merged the `fd9d5416` receipts (`08ed81546`); both of that issue's review gates failed.
- `git diff --shortstat 96bd8727f HEAD -- crates dev/tools` gives the session's source line counts.

## What to do next

- [ ] Dispatch `545b113f`. The scan that lists its scope, from the repository root:

  ```sh
  for f in $(git ls-files 'crates/**/*.rs' 'dev/tools/**/*.rs'); do
    awk -v f="$f" '/^\/\/!/{c++; next} {if(c)exit} END{if(c>12) print c, f}' "$f"
  done | sort -rn
  ```

  Brief it with `ffc35b8c-sweep-brief-template.md` (rule 3) and the public-item rule of `handoff-5.md`. Split by crate if the diff is large; the BCH files are the bulk.
- [ ] After `545b113f` merges: rerun the commands of `030496bd-sweep-completion.md` at the new measured commit (its REQ-05), then the command in `554c2935-citation-map.md`, and commit both outputs.
- [ ] Reconcile `surfaced_pitfalls` against REQ-21 to REQ-28, then run the `ffc35b8c` story gates (executable gates before `holistic-review`).
- [ ] Apply the owner's answers to the two open questions.
- [ ] Then waves 11 onward per `progress.json`.

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-5.md` remain in force. New this session:

- **Do NOT leave an owner ruling on the story alone.** `code-review` of `13dfe1a5` read the issue's own Decisions section and failed the collapsed `if` that story DEC-01 admits. Record each ruling as a DEC on the issue whose commits carry the hunk.
- **Do NOT pass decision text that starts with `- ` to `jit issue update --append-description`.** The CLI reads it as a flag and writes nothing; use `--append-description-file`.
- **Do NOT wait on a process with `pgrep -f` in a loop.** The session hook blocks it. Run each gate batch as its own background command.
- **Do NOT rebase or squash a branch whose record names a commit of that branch as its measured commit.** `030496bd` measures at `1078dc1a5`; merge with `--no-ff`.
- **Do NOT run `dispatch-worker-worktree.sh` while another session has uncommitted tracker files on main.** It refuses a dirty main; ask that session to commit by explicit path, or add the worktree with `git worktree add -b worktree-agent-<id> .agents/worktrees/agent-<id> HEAD` when the worker builds nothing.
- **Do NOT trust per-unit review for a story-wide criterion.** Every sweep unit passed review while 42 module docs stayed over twelve lines. Scan the whole tree against each story criterion before the story gate.
- **Do NOT write a criterion from memory of an API.** The first text of `10a729d3` required a result for empty input, where `batch_gcd` panics. Read the item first.
- **Do NOT edit under `dev/active/fd9d5416/` or merge for session `gf2-4e`.** Its permission classifier denied those actions; they are the owner's to approve.

## Open questions needing invoker input

- Question: who performs the `fd9d5416` rework, and does `3fd3db5e` fold into it?
  - Context: `fd9d5416` (epic `ae03bcd0`) failed `research-review` and `doc-review`; doc-review F2 is the literal `dev/` paths that `3fd3db5e` removes, F1 is the move the migration manifest plans, and `3fd3db5e` REQ-01 forbids edits while `fd9d5416` is in progress.
  - Options: the `fd9d5416` session does the whole rework and `3fd3db5e` is rejected as absorbed; amend `3fd3db5e` REQ-01 so the overhaul fixes the paths and performs the move; keep both and wait.
  - Recommendation: the first option, one owner for the renderer while its issue is open.
- Question: how is the `library-first-generality` exception of `crates/gf2-coding/src/bch/dvb_t2/mod.rs` recorded?
  - Context: the sweep removed its `@/issue/1a8f6acd` citation from Rustdoc because reviewers class an issue address as planned work; the interview answer did not select recording it on `1a8f6acd`.
  - Options: record the exception on issue `1a8f6acd`; restore a citation in source in a form reviewers accept, by an owner DEC; accept that the exception is recorded nowhere.
  - Recommendation: the first option.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, traps, created_during_execution)
- Sweep worker brief: `ffc35b8c-sweep-brief-template.md`
- Sweep completion record: `030496bd-sweep-completion.md`
- Citation registry: `.jit/references.toml`; map: `554c2935-citation-map.md`

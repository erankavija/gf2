# Handoff — Overhaul documentation for research adoption (fa787f85) — session 12

**Date:** 2026-10-07
**Session number:** 12
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6), `handoff-6.md` (session 7), `handoff-7.md` (session 8), `handoff-8.md` (session 9), `handoff-9.md` (session 10), `handoff-10.md` (session 11)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 13 of 16. Wave 12 is closed. `9451498f` is blocked on one owner ruling; waves 14 to 16 hold `d6ebc4ec`, `c852d9b2`, `a0ec34f2`, `af84619e`, each behind it.
- Children summary for the wave plan: 132 done, 1 blocked, 4 pending, 2 rejected.
- Active claims: `9451498f` (`agent:jit-execution-lead`, claimed 2026-10-07 for its gates).
- Open escalations: one, on `9451498f` (below and in `progress.json` `escalations`).
- Worktrees of this epic: none. No branch of this epic holds an unmerged commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner rulings in four rounds, each recorded as a DEC on its issue and in `progress.json` `escalations`: frozen-document links are repointed with their digests; reproduced literals are regenerated; a literal stays where a receipt, ledger or committed digest pins it; links of documents an open campaign or a declaration pins stay baselined; text bound to a named commit keeps that commit's path; session records of the running 1a379447 session keep their citations; entry dbd8787d stays flat; epic REQ-11 admits a flat entry with a tracked move issue (DEC-01); `2e18b37c` REQ-04 requires runtime resolution.
- Done, every gate passed: `f79bcba3`, `b5b336e5`, `602652bf`, `198aafa3`, `7039d680`, `2e18b37c`, `cc060974`, `f6148d1e`, `f694f59a`.
- Entry a83583e0 lives under `dev/active/6dc81018-field-capability-dispatch/`; the 6dc81018 archive reran twice (`archive-rerun-results.md`).
- `cc060974` and `2e18b37c`: the scripts of entries 00dd43c3, c04dd4ac, 19513245, a203a23c and f8dd4dde take the root from git run in their own directory; scans `cc060974-scripts.txt`, `2e18b37c-scripts.txt`.
- Filed in this epic: `a0ec34f2` (manifest rows that no move or archive task executes; `af84619e` depends on it).
- Filed under `b4b4b9ee`: `82ffd7b8`, `faac6007`, `d0aaf09b`, `5406d020`, `dfff0e05`, `7375db1e`, `954537d9`, `e804b8b8`; `progress.json` `created_during_execution` states each reason.
- `9451498f`: the primary checkout holds no empty or cache-only directory under `dev/active/`. `cargo-ci`, `docs-mechanical` and `doc-review` pass; `code-review` fails on REQ-02.

## What to do next

- [ ] Apply the owner's answer on `9451498f`, rerun `code-review`, close it.
- [ ] `d6ebc4ec`: profile-owned policy edits follow the capture, version bump and upgrade sequence of `handoff.md`.
- [ ] `c852d9b2`: `jit doc check-links --scope all` reports errors on main; read them before dispatch.
- [ ] `a0ec34f2`, then `af84619e`. `migration/check.py` prints the pending rows per disposition.
- [ ] Before the epic gate, reconcile `surfaced_pitfalls`: the test-written receipt at the top level of `dev/bench_results` against REQ-14, the `AGENTS.md` rows of the audit against REQ-02 and REQ-17, and the baselined links against REQ-19 (owner rulings of 2026-10-07 admit them on the units; the epic criterion keeps its text).
- [ ] Delete the merged `worktree-agent-*` branches of this epic before the next worktree dispatch.

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-10.md` remain in force. New this session:

- **Do NOT continue an existing subagent with a message.** The owner ruled that its cache is invalid; dispatch a fresh agent that reads the earlier return file.
- **Do NOT stop for a question in a session the owner marks as unattended.** Block the unit, record the escalation in `progress.json`, continue with other work.
- **Do NOT tag a tracker state commit with a unit's id.** `9451498f` code-review attributed `.jit/` changes to the unit; tag state commits `jit:fa787f85`.
- **Do NOT add a record to a unit whose criterion admits no tracked change.** `9451498f` failed both reviews on its own record.
- **Do NOT cite an issue as `<id> (jit issue show <id>)` in a record.** Three units failed review; records use `@/issue/<id>`. Commit bodies keep the command form.
- **Do NOT leave a `dev/` path literal in a unit's evidence script, a search pattern included.** `198aafa3` failed twice; derive the active root from the tracked files of an entry (`198aafa3-builds.sh`).
- **Do NOT accept "the root resolves at runtime" for a script that asks git from the caller's directory.** `cc060974` failed twice and `2e18b37c` once; the accepted form is `git -C <script directory>`, defaults joined to the root, output equal from the root and from `/tmp`.
- **Do NOT brief an edit of a pinned file without the pin check.** Checkpoint identities of a queued campaign, trial ledgers and the extent validator compare live digests; the workers found each before editing because the brief made it a stop condition.
- **Do NOT keep a helper script whose inputs lie outside the tree.** `f6148d1e-held-facts.py` read a stage under `/tmp`; the record cites the validator functions.
- **Do NOT edit a closed unit's verifier to follow a later move.** Its output is tool output at the commit of its last change; the later record states that ground.
- **Do NOT write `<outfile>` in a fenced command.** Shell reads it as redirection; `602652bf` doc-review failed on it.
- **Do NOT read exit status 254 from `sccache` as a compile error.** `cargo-ci` failed for `9451498f` and for the other session; restart the server and rerun.
- **Do NOT `git add .jit` while another session commits on main.** Stage tracker files by explicit path.
- **Parallel Bash calls that each `cd` race on the session directory.** Send one command per message when it changes directory, with an absolute `cd`.

## Open questions needing invoker input

- Question: how is `9451498f` REQ-02 ("No tracked file changes") read?
  - Context: the unit's net tracked diff outside `.jit/` is empty; `code-review` fails on `.jit/issues/9451498f-*.json`, the tracker state that claims and gate runs write.
  - Options: amend REQ-02 to "No tracked file outside the tracker changes"; record a DEC that tracker state is outside REQ-02; reject the finding and close.
  - Recommendation: the amendment.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, rework_counts, created_during_execution)
- Worker brief for move units: `wave11-worker-brief.md`
- Unit records of this session: `f79bcba3-zen3-move.md`, `602652bf-protocol-inputs-move.md`, `198aafa3-zen3-move.md`, `7039d680-fixture-entries-move.md`, `2e18b37c-zen3-move.md`, `cc060974-residual-shift-move.md`, `f6148d1e-held-entry.md`, `f694f59a-declaration-entry-move.md`
- Link baseline: `8f61d6de-baseline-findings.md`; archive evidence: `archive-rerun-results.md`
- Migration manifest and checker: `migration/manifest.toml`, `migration/check.py`

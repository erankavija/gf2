# Worker brief for move and archive units (epic fa787f85)

Binding for every wave 11 worker. `<ID>` is your issue's short id. `MAIN` is
the primary checkout, whose absolute path the dispatch prompt gives. Your worktree is the only
place you write.

## Contract

- Your contract is the live issue: `(cd MAIN && jit issue show <ID>)`. Every
  `[hard]` criterion is read literally. A criterion you cannot meet as written
  is reported under **Needs decision** with options and a recommendation; finish
  everything else. Ask nobody anything; do not wait for answers.
- Resolve cited addresses with `(cd MAIN && jit item show <address>)`.
- The issue's gates are `cargo-ci`, `code-review`, `doc-review` and
  `docs-mechanical`. The lead runs them on main after merging your branch; your
  work must be sufficient to pass them. Reviewers judge the whole diff of every
  commit tagged `jit:<ID>` against the engineering invariants in `AGENTS.md`.

## Read first

- `AGENTS.md` (root).
- `dev/active/fa787f85-documentation-overhaul/plan.md`, sections
  `migration-manifest`, `relocation-protocol`, `archive-execution-protocol`,
  `active-layout`.
- `dev/active/fa787f85-documentation-overhaul/migration/` (`manifest.toml`,
  `check.py`, `check_test.py`). Manifest rows are single lines of many
  kilobytes: never `cat`, `grep` or `sed -n` the file raw. Parse it with Python
  and print selected fields only. Edit rows by a Python script that changes
  only the fields you own (`status`, and `destination` when the plan requires)
  and leaves every other byte of the file intact.
- `dev/active/fa787f85-documentation-overhaul/investigation.md` where your
  issue cites it.

## Tracker

- The worktree `.jit` is a dispatch-time snapshot and refuses writes. Read live
  state with read-only commands from MAIN: `(cd MAIN && jit issue show ...)`,
  `jit doc list`, `jit item show`, `jit archive container <id> --json` (preview;
  never `--execute`).
- Never run a state-changing `jit` command. Never edit `.jit/`. Never create or
  change a file under MAIN outside your worktree.
- Tracker document-reference changes go into an idempotent script
  `target/<ID>-relink.sh`, uncommitted, that the lead runs on main after the
  merge. Form per reference, preserving doc type and label read from
  `jit doc list` / the issue JSON:
  `jit doc add <issue> <new-path> --doc-type <type> --label '<label>'` then
  `jit doc remove <issue> <old-path>`. End the script with
  `jit doc check-links --scope all`.

## Files

- Moves use `git mv`. History stays.
- Bytes that never change: receipt directories, preregistrations, addenda,
  digest-cited plans, journals, schemas, continuation JSON, snapshot copies
  (`inputs/` trees inside receipt directories) and every manifest row with
  `digest_pinned = true`. Receipts quote old paths as provenance strings; those
  strings stay.
- Never edit under `dev/active/fd9d5416/`.
- Other sessions work in this repository. Before moving or editing a path, check
  that no active worker branch carries unmerged commits touching it:
  `git log --oneline main..<branch> -- <path>` for the branches of
  `git worktree list`. A path with such commits stays in place and is reported.
- In historical artifacts only link targets change. Add no prose to them, do not
  reflow, do not fix unrelated defects.
- Rust sources: a `dev/active/` or `dev/archive/` path literal in Rustdoc or a
  module doc fails `no-dev-path-coupling`, and an `@/issue/` address there reads
  as planned work. Where a Rust comment cites a path you move, replace the path
  with a short statement of what the artifact is, or delete the citation when it
  is design-history provenance. After any `.rs` edit run
  `./scripts/cargo-budget.sh cargo fmt --all -- --check`. Do not build or test;
  the lead runs `cargo-ci`.
- Net line change is reviewed: the smallest diff that meets the criteria. No new
  explanatory prose outside your one record.

## Records

- One record per issue under `dev/active/fa787f85-documentation-overhaul/`,
  named `<ID>-<slug>.md`; scripts and raw outputs that back it carry the same
  `<ID>-` prefix and sit beside it. A committed script must reproduce its
  committed output at the commit the record names.
- Present tense, current state only. Forbidden: "pending", "will", "later",
  "once X", "to be", "previously", "now", "new", "old" as narration, any future
  promise, any marketing adjective. Tables over prose. State exactly what each
  check establishes and what it does not.
- A universal statement ("every", "all", "no") must hold literally; scan the
  whole scope before writing one.
- Name every record and script you add in your return; the lead links them.

## Checks before you return

- `python3 contrib/gates/docs-mechanical.py` passes.
- `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` (read
  its usage first) passes for the rows you changed.
- Every local link in each file you touched and each directory you moved
  resolves (the record names the command).
- `git status --short` shows nothing outside `target/`.

## Commits

- Subject `<type>(jit:<ID>): <summary>`; type is one of `feat`, `fix`, `docs`,
  `test`, `refactor`, `perf`, `chore`. The whole subject is at most 71
  characters: check with `s='...'; echo ${#s}`.
- Body: no bare short ids; write `<statement> -> <id> (jit issue show <id>)`.
  Name each path-literal edit inside a moved script, per entry.
- End each message with
  `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`.
- Commit each coherent step before starting the next. Never rebase, amend a
  commit an earlier return named, `git checkout`, `git switch` or push.

## Shell (zsh)

- Quote heredocs: `<<'EOF'`. Write `"${VAR}:path"`, never `$VAR:path`.
- Scratch files live in `target/` and start with `<ID>-`.

## Return

Write the full return to `target/<ID>-return.md` and repeat it as your final
message. Sections in order, `None.` when empty:

1. **Outcome** — `done`, `partial`, or `blocked`, then the deliverables your
   assignment names (artifact paths, commit SHAs, counts, verdict).
2. **Needs decision** — each choice, escalation, or conflict the dispatcher must
   resolve, with the options and your recommendation.
3. **Deviations** — each departure from the assignment, each unmet criterion, and
   each failed or skipped check with its command and first failing line.

Omit passing checks, step narration, restated instructions, and anything the
written artifact or commit already records. Stay within 200 words unless a
deviation needs more evidence.
